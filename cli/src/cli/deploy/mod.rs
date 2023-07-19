use clap::{Subcommand, Parser};

use crate::deploy::{registry::RainNetworks, deploy_contract};    


/// CLI utility to cross deploy Rain Contracts. 
#[derive(Subcommand)]
pub enum CrossDeploy{
    /// Cross Deploy a Rain Contract 
    DeployConsumer(Consumer)
}

#[derive(Parser, Debug)]
pub struct Consumer{

    /// origin network to deploy contract from
    #[arg(short, long = "from-network")]
    pub origin_network: RainNetworks,  

    /// target network to dpeloy contract
    #[arg(short, long = "to-network")]
    pub to_network: RainNetworks ,

    /// origin network interpreter address
    #[arg(short ='i' , long = "from-interpreter")]
    pub from_interpreter: Option<String>,

    /// origin network store address
    #[arg(short ='s' , long = "from-store")]
    pub from_store: Option<String>,

    /// origin network expression deployer address
    #[arg(short ='d' , long = "from-deployer")]
    pub from_deployer: Option<String>, 

    /// target network interpreter address
    #[arg(short ='I' , long = "to-interpreter")]
    pub to_interpreter: Option<String>,

    /// target network store address
    #[arg(short ='S' , long = "to-store")]
    pub to_store: Option<String>,

    /// target network expression deployer address
    #[arg(short ='D' , long = "to-deployer")]
    pub to_deployer: Option<String>,

    /// origin network contract address
    #[arg(short ='c' , long = "contract-address")]
    pub contract_address: String ,

    /// origin network transaction hash to source data from
    #[arg(short ='H' , long = "transaction-hash")]
    pub transaction_hash: Option<String> ,

    /// Set to true to deploy contract to target network 
    #[arg(long)]
    pub deploy: bool, 

    /// private key (unprefixed) provided when deploy is set to true
    #[arg(short ='k' , long = "priavte-key" )]
    pub private_key: Option<String>,
} 

/// CLI function handler
pub async fn deploy(cross_deploy: CrossDeploy) -> anyhow::Result<()> {
     match cross_deploy {
        CrossDeploy::DeployConsumer(consumer) => {  
            deploy_contract(consumer).await?              
        }
    } ;  
    Ok(())

}
