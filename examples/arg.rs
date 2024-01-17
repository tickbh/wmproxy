
// use bpaf::*;

// #[derive(Debug, Clone, Bpaf)]
// #[allow(dead_code)]
// #[bpaf(options("hackerman"))]
// struct All {
//     #[bpaf(long, short, argument("CRATE"))]
//     pub fff: String,
//     /// aaaaaaaaaaaaadfwerwe
//     #[bpaf(long, short, argument("SPEC fddsa"))]
//     pub xxx: Option<String>,
// }

// #[derive(Debug, Clone, Bpaf)]
// #[bpaf(options("hackerman"))]
// pub enum Action {
//     #[bpaf(command("explain"))]
//     Explain {
//         #[bpaf(long, short, argument("CRATE"))]
//         krate: String,
//         /// aaaaaaaaaaaaadfwerwe
//         #[bpaf(long, short, argument("SPEC fddsa"))]
//         feature: Option<String>,
//         #[bpaf(external(version_if))]
//         version: Option<String>,
//     },
//     #[bpaf(command("global"))]
//     Global {
//         #[bpaf(positional("CRATE"))]
//         krate: String,
//     }
// }

// fn feature_if() -> impl Parser<Option<String>> {
//     // here feature starts as any string on a command line that does not start with a dash
//     positional::<String>("FEATURE")
//         // guard restricts it such that it can't be a valid version
//         .guard(move |s| !is_version(s), "")
//         // last two steps describe what to do with strings in this position but are actually
//         // versions.
//         // optional allows parser to represent an ignored value with None
//         .optional()
//         // and catch lets optional to handle parse failures coming from guard
//         .catch()
// }

// fn version_if() -> impl Parser<Option<String>> {
//     positional::<String>("VERSION")
//         .guard(move |s| is_version(s), "")
//         .optional()
//         .catch()
// }

// fn is_version(v: &str) -> bool {
//     v.chars().all(|c| c.is_numeric())
// }


// fn parse_command() -> impl Parser<(Action, All)> {
//     let action = action().map(Action::Explain);
//     let action = construct!(action, shared()).to_options().command("action");
//     let build = build().map(Command::Build);
//     let build = construct!(build, shared()).to_options().command("build");
//     construct!([action, build])
// }

// fn main() {
//     let action = action().map(Command::Action);
//     let action = construct!(action, shared()).to_options().command("action");

//     let vals = construct!(action(), all());
//     println!("{:?}", action().fallback_to_usage().run());
// }



use bpaf::*;

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
#[bpaf(options("hackerman"))]
struct All {
    #[bpaf(long, short, argument("CRATE"))]
    pub fff: String,
    /// aaaaaaaaaaaaadfwerwe
    #[bpaf(long, short, argument("SPEC fddsa"))]
    pub xxx: Option<String>,
}

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct Action {
    verbose: bool,
    number: u32,
}

#[derive(Debug, Clone, Bpaf)]
#[allow(dead_code)]
struct Build {
    verbose: bool,
}

#[derive(Debug, Clone)]
enum Command {
    Action(Action),
    Build(Build),
}

fn speed() -> impl Parser<f64> {
    long("speed")
        .help("Speed in KPH")
        .argument::<f64>("SPEED")
}

fn parse_command() -> impl Parser<(Command, f64)> {
    let action = action().map(Command::Action);
    let _val = all().command("all");
    let action = construct!(action, speed()).to_options().command("action");
    let build = build().map(Command::Build);
    let build = construct!(build, speed()).to_options().command("");
    construct!([action, build])
}

fn main() {
    let opts = parse_command().to_options().run();

    println!("{:?}", opts);
}