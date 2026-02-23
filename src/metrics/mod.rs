use crate::state::AppState;

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

pub async fn render_metrics(state: &AppState) -> String {
    let mut lines = Vec::new();

    lines.push("# HELP evolution_environment_info Environment information".to_string());
    lines.push("# TYPE evolution_environment_info gauge".to_string());
    lines.push(format!(
        "evolution_environment_info{{version=\"{}\",clientName=\"{}\",serverUrl=\"{}\"}} 1",
        escape_label(env!("CARGO_PKG_VERSION")),
        escape_label(&state.config.database.client_name),
        escape_label(&state.config.server.url),
    ));

    let instances = state.wa_instances.read().await;
    lines.push("# HELP evolution_instances_total Total number of instances".to_string());
    lines.push("# TYPE evolution_instances_total gauge".to_string());
    lines.push(format!("evolution_instances_total {}", instances.len()));

    lines.push("# HELP evolution_instance_up 1 if instance state is open, else 0".to_string());
    lines.push("# TYPE evolution_instance_up gauge".to_string());
    lines.push("# HELP evolution_instance_state Instance state as a labelled metric".to_string());
    lines.push("# TYPE evolution_instance_state gauge".to_string());

    for (name, instance) in instances.iter() {
        let up = if instance.state == "open" { 1 } else { 0 };
        lines.push(format!(
            "evolution_instance_up{{instance=\"{}\",integration=\"{}\"}} {}",
            escape_label(name),
            escape_label(&instance.integration),
            up,
        ));
        lines.push(format!(
            "evolution_instance_state{{instance=\"{}\",integration=\"{}\",state=\"{}\"}} 1",
            escape_label(name),
            escape_label(&instance.integration),
            escape_label(&instance.state),
        ));
    }

    lines.join("\n") + "\n"
}
