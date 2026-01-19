use crate::domain::catalog::Catalog;
use crate::domain::resource::{MetaKind, Resource};
use anyhow::Result;
use colored::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphStyle {
    Ascii,
    Mermaid,
}

pub fn display_graph(
    catalog: &Catalog,
    filter: Option<&[String]>,
    max_depth: usize,
    style: GraphStyle,
) -> Result<()> {
    match style {
        GraphStyle::Ascii => render_ascii(catalog, filter, max_depth),
        GraphStyle::Mermaid => render_mermaid(catalog, filter, max_depth),
    }
}

fn render_ascii(catalog: &Catalog, filter: Option<&[String]>, max_depth: usize) -> Result<()> {
    println!("\n{}", "Dependency Graph:".bold().underline());

    let filter_set: Option<HashSet<String>> = filter.map(|f| f.iter().cloned().collect());

    // 1. Build a map of resources for easy lookup
    let resource_map: HashMap<String, &Resource> = catalog
        .evaluation_order
        .iter()
        .map(|r| (r.id().to_string(), r))
        .collect();

    // 2. Identify children for each parent (ModuleStart)
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut top_level: Vec<String> = Vec::new();

    for resource in &catalog.evaluation_order {
        let mut parent_found = false;
        let parent_id = resource
            .dependencies()
            .iter()
            .find(|d| d.contains("Start["));

        if let Some(p_id) = parent_id {
            children_map
                .entry(p_id.to_string())
                .or_default()
                .push(resource.id().to_string());
            parent_found = true;
        }

        if !parent_found && !resource.id().contains("End[") {
            top_level.push(resource.id().to_string());
        }
    }

    // 3. Print recursively starting from top-level
    for id in &top_level {
        if let Some(res) = resource_map.get(id) {
            print_node_ascii(res, &resource_map, &children_map, &filter_set, 0, max_depth);
        }
    }

    Ok(())
}

fn render_mermaid(catalog: &Catalog, filter: Option<&[String]>, max_depth: usize) -> Result<()> {
    println!("\n```mermaid");
    println!("graph TD");

    let filter_set: Option<HashSet<String>> = filter.map(|f| f.iter().cloned().collect());

    // 1. Build a map of resources for easy lookup
    let resource_map: HashMap<String, &Resource> = catalog
        .evaluation_order
        .iter()
        .map(|r| (r.id().to_string(), r))
        .collect();

    // 2. Identify children for each parent (ModuleStart)
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut top_level: Vec<String> = Vec::new();

    for resource in &catalog.evaluation_order {
        let mut parent_found = false;
        let parent_id = resource
            .dependencies()
            .iter()
            .find(|d| d.contains("Start["));

        if let Some(p_id) = parent_id {
            children_map
                .entry(p_id.to_string())
                .or_default()
                .push(resource.id().to_string());
            parent_found = true;
        }

        if !parent_found && !resource.id().contains("End[") {
            top_level.push(resource.id().to_string());
        }
    }

    // 3. Render nodes and hierarchy arrows
    for id in &top_level {
        if let Some(res) = resource_map.get(id) {
            render_node_mermaid(
                res,
                None,
                &resource_map,
                &children_map,
                &filter_set,
                0,
                max_depth,
            );
        }
    }

    println!("```");
    Ok(())
}

fn print_node_ascii(
    resource: &Resource,
    resource_map: &HashMap<String, &Resource>,
    children_map: &HashMap<String, Vec<String>>,
    filter: &Option<HashSet<String>>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    match resource {
        Resource::Meta(meta) => {
            if meta.kind == MetaKind::ModuleStart {
                let (kind, name) = parse_meta_id(&meta.id);
                print_hierarchy_item_ascii(&kind, &name, depth);

                if let Some(children) = children_map.get(&meta.id) {
                    let next_depth = depth + 1;
                    for child_id in children {
                        if let Some(child_res) = resource_map.get(child_id) {
                            print_node_ascii(
                                child_res,
                                resource_map,
                                children_map,
                                filter,
                                next_depth,
                                max_depth,
                            );
                        }
                    }
                }
            }
        }
        _ => {
            if is_filtered(resource, filter) {
                print_resource_item_ascii(resource, depth);
            }
        }
    }
}

fn render_node_mermaid(
    resource: &Resource,
    parent_safe_id: Option<String>,
    resource_map: &HashMap<String, &Resource>,
    children_map: &HashMap<String, Vec<String>>,
    filter: &Option<HashSet<String>>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }

    let safe_id = sanitize_id(resource.id());

    match resource {
        Resource::Meta(meta) => {
            if meta.kind == MetaKind::ModuleStart {
                let (kind, name) = parse_meta_id(&meta.id);
                let label = format!("{}: {}", kind, name);

                let node_syntax = match kind.as_str() {
                    "Role" => format!("{safe_id}{{{{{label}}}}}"),
                    "Profile" => format!("{safe_id}[[{label}]]"),
                    "Module" => format!("{safe_id}(\"{label}\")"),
                    _ => format!("{safe_id}(\"{label}\")"),
                };

                println!("  {}", node_syntax);

                if let Some(p_id) = parent_safe_id {
                    println!("  {} ==> {}", p_id, safe_id);
                }

                if let Some(children) = children_map.get(&meta.id) {
                    let next_depth = depth + 1;
                    for child_id in children {
                        if let Some(child_res) = resource_map.get(child_id) {
                            render_node_mermaid(
                                child_res,
                                Some(safe_id.clone()),
                                resource_map,
                                children_map,
                                filter,
                                next_depth,
                                max_depth,
                            );
                        }
                    }
                }
            }
        }
        _ => {
            if is_filtered(resource, filter) {
                let id = resource.id();
                println!("  {}[\"{}\"]", safe_id, id);

                if let Some(p_id) = parent_safe_id {
                    println!("  {} ==> {}", p_id, safe_id);
                }

                // Render internal resource dependencies in Mermaid
                let deps = resource.dependencies();
                for dep in deps {
                    if !dep.contains("Start[") && !dep.contains("End[") {
                        let safe_dep_id = sanitize_id(dep);
                        println!("  {} --> {}", safe_id, safe_dep_id);
                    }
                }
            }
        }
    }
}

fn sanitize_id(id: &str) -> String {
    id.replace("[", "_")
        .replace("]", "_")
        .replace("/", "_")
        .replace(".", "_")
        .replace("-", "_")
        .replace(" ", "_")
        .replace(":", "_")
        .replace(">", "_")
}

fn is_filtered(resource: &Resource, filter: &Option<HashSet<String>>) -> bool {
    if let Some(set) = filter {
        let type_name = match resource {
            Resource::File(_) => "file",
            Resource::Directory(_) => "directory",
            Resource::Exec(_) => "exec",
            _ => "",
        };
        set.contains(type_name)
    } else {
        true
    }
}

fn parse_meta_id(id: &str) -> (String, String) {
    if id.starts_with("RoleStart[") {
        ("Role".to_string(), id[10..id.len() - 1].to_string())
    } else if id.starts_with("ProfileStart[") {
        ("Profile".to_string(), id[13..id.len() - 1].to_string())
    } else if id.starts_with("ModuleStart[") {
        ("Module".to_string(), id[12..id.len() - 1].to_string())
    } else {
        ("Unknown".to_string(), id.to_string())
    }
}

fn print_hierarchy_item_ascii(kind: &str, name: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let prefix = match kind {
        "Role" => "Role:".cyan().bold(),
        "Profile" => "Profile:".magenta().bold(),
        "Module" => "Module:".yellow().bold(),
        _ => format!("{}:", kind).white().bold(),
    };

    println!("{}└─ {} {}", indent, prefix, name.bright_white());
}

fn print_resource_item_ascii(resource: &Resource, depth: usize) {
    let indent = "  ".repeat(depth);
    let id = resource.id();

    let color_fn = match resource {
        Resource::File(_) => Color::White,
        Resource::Directory(_) => Color::Blue,
        Resource::Exec(_) => Color::Green,
        _ => Color::Red,
    };

    println!("{}├─ {}", indent, id.color(color_fn));

    let deps = resource.dependencies();
    if !deps.is_empty() {
        let user_deps: Vec<_> = deps
            .iter()
            .filter(|d| !d.contains("Start[") && !d.contains("End["))
            .collect();

        if !user_deps.is_empty() {
            let dep_indent = format!("{}│  ", indent);
            for dep in user_deps {
                println!("{}└─→ {}", dep_indent, dep.dimmed());
            }
        }
    }
}
