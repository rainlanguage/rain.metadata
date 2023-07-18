use graphql_client::{GraphQLQuery, Response};
use reqwest;
use anyhow::{Result, Ok};
use crate::deploy::registry::{RainNetworks, Ethereum, Polygon, Mumbai, Fuji};
use serde::{Deserialize, Serialize};
use anyhow::anyhow;


#[derive(GraphQLQuery, Debug)]
#[graphql(
    schema_path = "src/subgraph/schema.json",
    query_path = "src/subgraph/query.graphql",
    response_derives = "Debug"
)]
pub struct ContractQuery;  
 

pub async fn get_transaction_hash( 
    network : &RainNetworks ,
    contract_address : &String
) -> Result<String> { 

    let variable = contract_query::Variables {
        addr: Some(contract_address.to_string()),
    };

    let request_body = ContractQuery::build_query(variable);
    let client = reqwest::Client::new(); 

    let url = match &network {
        RainNetworks::Polygon => {
            Polygon::default().url
        },  
        RainNetworks::Ethereum => {
            Ethereum::default().url
        }
        RainNetworks::Mumbai => {
            Mumbai::default().url
        },
        RainNetworks::Fuji => {
            String::from("")
        }
    } ; 
 
    let res: reqwest::Response = client
        .post(url)
        .json(&request_body)
        .send()
        .await?; 

    let response_body: Response<contract_query::ResponseData> = res.json().await?;  

    let query_contract = response_body
        .data.unwrap().contract ; 

    match query_contract {
        Some(contract_query) => {
            let tx_hash = contract_query.deploy_transaction.unwrap().id ;
            Ok(tx_hash) 
        } 
        None => { 
            let hash = get_scan_transaction_hash(network,contract_address).await? ; 
            Ok(hash)
        }
    } 
    
}  
 
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
 struct ContractData {
    contract_address : String ,
    contract_creator : String ,
    tx_hash : String ,
 }
 #[derive(Serialize, Deserialize, Debug)]
 #[serde(rename_all = "camelCase")]
struct ContractCreation{
    message : String , 
    status : String,  
    result : Vec<ContractData>
}

pub async fn get_scan_transaction_hash(
    network : &RainNetworks ,
    contract_address : &String
) -> Result<String> {  

    let ( scan_url,  api_key) = match network {
        RainNetworks::Polygon => {
            (
                Polygon::default().block_scanner_api,
                Polygon::default().block_scanner_key,

            )
        },  
        RainNetworks::Ethereum => {
            (
                Ethereum::default().block_scanner_api,
                Ethereum::default().block_scanner_key,

            )
        }
        RainNetworks::Mumbai => {
            (
                Mumbai::default().block_scanner_api,
                Mumbai::default().block_scanner_key,

            )
        },
        RainNetworks::Fuji => {
            (
                Fuji::default().block_scanner_api,
                Fuji::default().block_scanner_key,

            )
        }
    } ;  

     let url = format!(
        "{}{}{}{}{}",
        scan_url,
        String::from("api?module=contract&action=getcontractcreation&contractaddresses="),
        contract_address,
        String::from("&apikey=") ,
        api_key
     );  

     let res = reqwest::Client::new().get(url).send().await? ; 
     let body: String = res.text().await?;   
     let response_body: std::result::Result<ContractCreation, serde_json::Error> = serde_json::from_str::<ContractCreation>(&body) ;  
     
    match response_body {
         std::result::Result::Ok(val) => {
            let hash = &val.result[0].tx_hash ;
            return Ok(hash.to_string()) ;
         } ,
         Err(_) => {
            return Err(anyhow!("\n❌ Contract not found.\n Try providing a transaction hash")) ;
         } ,
     };  

}  

#[cfg(test)] 
mod test { 

    use super::get_transaction_hash ;
    use crate::deploy::registry::RainNetworks;

    #[tokio::test]
    async fn test_subgraph_contract_address()  {
        let from_network = RainNetworks::Mumbai ; 
        let contract_address = String::from("0x3cC6C6E888B4Ad891EEA635041A269C4BA1c4A63") ;   
        let tx_hash = get_transaction_hash(&from_network,&contract_address).await.unwrap() ;  
        let expected_hash = String::from("0xc215bf3dc7440687ca20e028158e58640eeaec72d6fe6738f6d07843835c2cde") ;
        assert_eq!(tx_hash,expected_hash) ;
    }  

    #[tokio::test]
    async fn test_scan_contract_address()  {
        let from_network = RainNetworks::Mumbai ; 
        let contract_address = String::from("0x2c9f3204590765aefa7bee01bccb540a7d06e967") ;   
        let tx_hash = get_transaction_hash(&from_network,&contract_address).await.unwrap() ;  
        let expected_hash = String::from("0xea76ed73832498c4293aa06aeca2899f2b5adca15d703b03690185ed829f3e71") ;
        assert_eq!(tx_hash,expected_hash) ;
    }  

    #[tokio::test]
    async fn test_get_hash_fail()  {
        let from_network = RainNetworks::Mumbai ; 
        let contract_address = String::from("0x00000000000000000000000") ;   
        let tx_hash = get_transaction_hash(&from_network,&contract_address).await ;  
        assert!(tx_hash.is_err()) ;
    } 

}