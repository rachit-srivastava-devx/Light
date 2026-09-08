//! Arg groups for contract/gate/freeze/context/mcp subcommands. Same pragmatic-subset caveat as
//! `args_core.rs`.

use clap::Args;

#[derive(Args, Debug)]
pub struct ContractArgs {
    #[arg(long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct GateArgs {
    #[arg(long)]
    pub id: Option<String>,
}

#[derive(Args, Debug)]
pub struct FreezeArgs {
    #[arg(long)]
    pub path: String,
}

#[derive(Args, Debug)]
pub struct GraphArgs {
    #[arg(long)]
    pub repo: String,
}

#[derive(Args, Debug)]
pub struct ImpactArgs {
    #[arg(long)]
    pub symbol: String,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    #[arg(long)]
    pub lease: String,
}
