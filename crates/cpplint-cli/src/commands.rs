use crate::options::{CheckArgs, RuleArgs};
use cpplint_core::registry::{rule_registry, RuleFamily, RuleSelection};
use cpplint_core::runner::run_lint;
use std::process::ExitCode;

pub fn run_check_command(args: &CheckArgs) -> ExitCode {
    let runner_config = match args.to_runner_config() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{}", message);
            return ExitCode::from(1);
        }
    };

    match run_lint(&args.files, &runner_config) {
        Ok(result) => {
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            ExitCode::from((result.error_count > 0) as u8)
        }
        Err(error) => {
            eprintln!("{}", error);
            ExitCode::from(1)
        }
    }
}

pub fn run_rule_command(args: &RuleArgs) -> ExitCode {
    let registry = rule_registry();

    if let Some(query) = &args.query {
        let Some(selection) = registry.select(query) else {
            eprintln!("Unknown rule or rule family: {}", query);
            return ExitCode::from(1);
        };
        print!("{}", render_selection(registry, selection));
        return ExitCode::SUCCESS;
    }

    print!("{}", render_registry_overview(registry, args.all));
    ExitCode::SUCCESS
}

fn render_registry_overview(
    registry: &cpplint_core::registry::RuleRegistry,
    include_categories: bool,
) -> String {
    let mut rendered = String::new();
    rendered.push_str("cpplint rule families\n");

    for family in registry.families() {
        rendered.push_str(&format!(
            "\n- {} ({})\n  {}\n",
            family.name,
            family.phase.as_str(),
            family.summary
        ));
        if include_categories {
            for category in family.categories {
                rendered.push_str(&format!("  * {}\n", category));
            }
        }
    }

    rendered.push_str(&format!(
        "\nSupported categories: {}\n",
        registry.all_categories().len()
    ));
    rendered
}

fn render_selection(
    registry: &cpplint_core::registry::RuleRegistry,
    selection: RuleSelection,
) -> String {
    match selection {
        RuleSelection::Family(family) => render_family(family),
        RuleSelection::Category { category, family } => {
            let mut rendered = String::new();
            rendered.push_str(&format!("Rule: {}\n", category));
            rendered.push_str(&format!("Family: {}\n", family.name));
            rendered.push_str(&format!("Phase: {}\n", family.phase.as_str()));
            rendered.push_str(&format!(
                "Summary: {} checks handled by the {} family.\n",
                registry.humanize_category(category),
                family.name
            ));
            rendered.push_str("Sibling categories:\n");
            for sibling in family.categories {
                rendered.push_str(&format!("  - {}\n", sibling));
            }
            rendered
        }
    }
}

fn render_family(family: RuleFamily) -> String {
    let mut rendered = String::new();
    rendered.push_str(&format!("Family: {}\n", family.name));
    rendered.push_str(&format!("Phase: {}\n", family.phase.as_str()));
    rendered.push_str(&format!("Summary: {}\n", family.summary));
    rendered.push_str("Categories:\n");
    for category in family.categories {
        rendered.push_str(&format!("  - {}\n", category));
    }
    rendered
}
