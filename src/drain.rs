use std::cell::RefCell;
use std::collections::HashMap;

const WILDCARD_TOKEN: &str = "<*>";

#[derive(Debug, Clone, PartialEq)]
pub struct DrainConfig {
    pub depth: usize,
    pub max_children: usize,
    pub similarity: f64,
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            depth: 4,
            max_children: 100,
            similarity: 0.4,
        }
    }
}

impl DrainConfig {
    pub fn sanitized(&self) -> Self {
        let depth = self.depth.max(2);
        let max_children = self.max_children.max(1);
        let similarity = self.similarity.clamp(0.0, 1.0);
        Self {
            depth,
            max_children,
            similarity,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrainTemplate {
    pub template: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct DrainResult {
    pub template: String,
    pub count: usize,
    pub is_new: bool,
}

#[derive(Debug)]
struct Cluster {
    template_tokens: Vec<String>,
    count: usize,
}

#[derive(Debug, Default)]
struct Node {
    children: HashMap<String, Node>,
    cluster_ids: Vec<usize>,
}

#[derive(Debug)]
struct Drain {
    config: DrainConfig,
    roots: HashMap<usize, Node>,
    clusters: Vec<Cluster>,
}

impl Drain {
    fn new(config: DrainConfig) -> Self {
        Self {
            config: config.sanitized(),
            roots: HashMap::new(),
            clusters: Vec::new(),
        }
    }

    fn ingest(&mut self, text: &str) -> DrainResult {
        let tokens: Vec<String> = text
            .split_whitespace()
            .map(|token| token.to_string())
            .collect();
        let token_count = tokens.len();
        let depth = self.config.depth.min(token_count.max(1));
        let config = &self.config;

        let root = self.roots.entry(token_count).or_default();
        let leaf = descend_to_leaf(root, &tokens, depth, config);
        let (cluster_id, is_new) =
            find_or_create_cluster(leaf, &tokens, &mut self.clusters, config);

        let cluster = &mut self.clusters[cluster_id];
        let template = cluster.template_tokens.join(" ");
        cluster.count += 1;

        DrainResult {
            template,
            count: cluster.count,
            is_new,
        }
    }

    fn templates(&self) -> Vec<DrainTemplate> {
        let mut templates: Vec<DrainTemplate> = self
            .clusters
            .iter()
            .map(|cluster| DrainTemplate {
                template: cluster.template_tokens.join(" "),
                count: cluster.count,
            })
            .collect();

        templates.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.template.cmp(&b.template))
        });

        templates
    }
}

fn descend_to_leaf<'a>(
    root: &'a mut Node,
    tokens: &[String],
    depth: usize,
    config: &DrainConfig,
) -> &'a mut Node {
    let mut node = root;
    for token in tokens.iter().take(depth.saturating_sub(1)) {
        let mut key = if is_variable_token(token) {
            WILDCARD_TOKEN
        } else {
            token.as_str()
        };

        if !node.children.contains_key(key) {
            if node.children.len() >= config.max_children && key != WILDCARD_TOKEN {
                key = WILDCARD_TOKEN;
            }
            node.children.entry(key.to_string()).or_default();
        }

        node = node.children.get_mut(key).expect("child node exists");
    }

    node
}

fn find_or_create_cluster(
    leaf: &mut Node,
    tokens: &[String],
    clusters: &mut Vec<Cluster>,
    config: &DrainConfig,
) -> (usize, bool) {
    if leaf.cluster_ids.is_empty() {
        let cluster_id = create_cluster(tokens, clusters);
        leaf.cluster_ids.push(cluster_id);
        return (cluster_id, true);
    }

    let mut best_id = None;
    let mut best_similarity = 0.0;
    let mut best_non_wildcards = 0usize;

    for &cluster_id in &leaf.cluster_ids {
        let cluster = &clusters[cluster_id];
        let (similarity, non_wildcards) = template_similarity(&cluster.template_tokens, tokens);
        if similarity > best_similarity
            || (similarity == best_similarity && non_wildcards > best_non_wildcards)
        {
            best_similarity = similarity;
            best_non_wildcards = non_wildcards;
            best_id = Some(cluster_id);
        }
    }

    if let Some(cluster_id) = best_id {
        if best_similarity >= config.similarity {
            update_template(cluster_id, tokens, clusters);
            return (cluster_id, false);
        }
    }

    let cluster_id = create_cluster(tokens, clusters);
    leaf.cluster_ids.push(cluster_id);
    (cluster_id, true)
}

fn create_cluster(tokens: &[String], clusters: &mut Vec<Cluster>) -> usize {
    let cluster = Cluster {
        template_tokens: tokens.to_vec(),
        count: 0,
    };
    clusters.push(cluster);
    clusters.len() - 1
}

fn update_template(cluster_id: usize, tokens: &[String], clusters: &mut [Cluster]) {
    let cluster = &mut clusters[cluster_id];
    for (idx, token) in tokens.iter().enumerate() {
        if let Some(existing) = cluster.template_tokens.get_mut(idx) {
            if existing != token {
                *existing = WILDCARD_TOKEN.to_string();
            }
        }
    }
}

fn is_variable_token(token: &str) -> bool {
    token.chars().any(|ch| ch.is_ascii_digit())
}

fn template_similarity(template: &[String], tokens: &[String]) -> (f64, usize) {
    let mut matches = 0usize;
    let mut non_wildcards = 0usize;

    for (templ, token) in template.iter().zip(tokens.iter()) {
        if templ == WILDCARD_TOKEN {
            continue;
        }
        non_wildcards += 1;
        if templ == token {
            matches += 1;
        }
    }

    let denom = template.len().max(1) as f64;
    (matches as f64 / denom, non_wildcards)
}

thread_local! {
    static DRAIN_STATE: RefCell<Option<Drain>> = const { RefCell::new(None) };
}

pub fn reset() {
    DRAIN_STATE.with(|state| {
        *state.borrow_mut() = None;
    });
}

pub fn drain_template(text: &str, config: Option<DrainConfig>) -> Result<DrainResult, String> {
    DRAIN_STATE.with(|state| {
        let mut state = state.borrow_mut();
        match (state.as_ref(), &config) {
            (None, Some(cfg)) => {
                *state = Some(Drain::new(cfg.clone()));
            }
            (None, None) => {
                *state = Some(Drain::new(DrainConfig::default()));
            }
            (Some(existing), Some(cfg)) => {
                let sanitized = cfg.sanitized();
                if existing.config != sanitized {
                    return Err("Drain config already initialized with different options".into());
                }
            }
            _ => {}
        }

        let drain = state
            .as_mut()
            .ok_or_else(|| "Drain state not initialized".to_string())?;
        Ok(drain.ingest(text))
    })
}

pub fn drain_templates() -> Vec<DrainTemplate> {
    DRAIN_STATE.with(|state| match state.borrow().as_ref() {
        Some(drain) => drain.templates(),
        None => Vec::new(),
    })
}

pub fn format_templates_output(templates: &[DrainTemplate]) -> String {
    if templates.is_empty() {
        return "No templates found".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("templates ({} items):\n", templates.len()));

    for (idx, template) in templates.iter().enumerate() {
        output.push_str(&format!(
            "  #{:<3} {:<40} {}\n",
            idx + 1,
            template.template,
            template.count
        ));
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clusters_similar_lines() {
        let mut drain = Drain::new(DrainConfig::default());
        let a = drain.ingest("failed to connect to 10.0.0.1");
        let b = drain.ingest("failed to connect to 10.0.0.2");

        assert_eq!(a.template, "failed to connect to 10.0.0.1");
        assert_eq!(b.template, "failed to connect to <*>");
        assert_eq!(b.count, 2);
    }

    #[test]
    fn formats_templates_output() {
        let templates = vec![
            DrainTemplate {
                template: "a <*> b".to_string(),
                count: 3,
            },
            DrainTemplate {
                template: "x y z".to_string(),
                count: 1,
            },
        ];
        let output = format_templates_output(&templates);
        assert!(output.starts_with("templates (2 items):"));
        assert!(output.contains("#1"));
    }
}
