use clap::Parser;

/// # DISpair
/// Interpreter, Store, Expression Deployer.
/// Contracts required to deploy any Rain Contract.
/// Any or all of these can be provided while cross deploying a contract.
/// If a contract is a non-Rain Contract the value can be None. 
#[derive(Parser,Clone)]
pub struct DISpair {
    pub interpreter : Option<String> ,
    pub store : Option<String> ,
    pub deployer : Option<String> ,
}  

impl DISpair {
    pub fn new(i : Option<String>, s : Option<String>, d : Option<String>) -> DISpair {
        DISpair { interpreter: i, store: s, deployer: d }
    }
}
 
/// Builds contract deployment transaction data by replacing the [DISpair] 
/// contract instances in the transaction data of the origin network, with
/// the [DISpair] instances of the target network. To replace any or all of the DIS contracts,
/// both the counterparties on origin and target network must be provided. 
/// 
/// # Example   
/// 
/// Get
/// 
/// ```rust
/// use rain_cli_meta::deploy::dis::replace_dis_pair;
/// use rain_cli_meta::deploy::transaction::get_transaction_data;
/// use rain_cli_meta::deploy::dis::DISpair; 
/// use rain_cli_meta::deploy::registry::RainNetworks; 
/// use rain_cli_meta::deploy::registry::Mumbai;
/// use std::env;
/// 
/// async fn replace_dis() {    
/// 
/// // Reading environment variables
/// let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
/// let network: RainNetworks = RainNetworks::Mumbai(mumbai_network);
/// 
/// // Origin network transaction hash
/// let tx_hash = String::from("0xc215bf3dc7440687ca20e028158e58640eeaec72d6fe6738f6d07843835c2cde") ; 
/// 
/// // Get origin network transaction data
/// let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ;  
/// 
/// // Initiate origin network DISpair instance
/// let from_dis = DISpair {
///     interpreter : Some(String::from("0x5f02c2f831d3e0d430aa58c973b8b751f3d81b38")) ,
///     store : Some(String::from("0xa5d9c16ddfd05d398fd0f302edd9e9e16d328796")) , 
///     deployer : Some(String::from("0xd3870063bcf25d5110ab9df9672a0d5c79c8b2d5"))
/// } ; 
///
/// // Initiate target network DISpair instance
/// let to_dis = DISpair {
///    interpreter : Some(String::from("0xfd1da7eee4a9391f6fcabb28617f41894ba84cdc")),
///    store : Some(String::from("0x9b8571bd2742ec628211111de3aa940f5984e82b")),  
///    deployer : Some(String::from("0x3d7d894afc7dbfd45bf50867c9b051da8eee85e9")),
/// } ; 
///
/// // Get contract deployment transaction data for the target network .
///  let contract_deployment_data = replace_dis_pair(
///     tx_data,
///     from_dis,
///     to_dis
///  ).unwrap() ; 
/// }
/// ```  

pub fn replace_dis_pair(
    tx_data : String ,
    from_dis : DISpair , 
    to_dis : DISpair
) -> Option<String> { 

   let mut ret_str = tx_data.to_lowercase() ;  

   // Both the counterparties should be provided
   if from_dis.interpreter.as_ref().is_some() && to_dis.interpreter.as_ref().is_some() {
        if tx_data.contains(&from_dis.interpreter.as_ref().unwrap()[2..].to_lowercase()){
            ret_str = ret_str.replace(&from_dis.interpreter.as_ref().unwrap()[2..].to_lowercase(), &to_dis.interpreter.as_ref().unwrap()[2..].to_lowercase()) ; 
        }
   } 
   // Both the counterparties should be provided
   if from_dis.store.is_some() && to_dis.store.is_some() {
        if tx_data.contains(&from_dis.store.as_ref().unwrap()[2..].to_lowercase()){
            ret_str = ret_str.replace(&from_dis.store.as_ref().unwrap()[2..].to_lowercase(), &to_dis.store.as_ref().unwrap()[2..].to_lowercase()) ; 
        }
   }
   // Both the counterparties should be provided
   if from_dis.deployer.is_some() && to_dis.deployer.is_some() { 
        if tx_data.contains(&from_dis.deployer.as_ref().unwrap()[2..].to_lowercase()){
            ret_str = ret_str.replace(&from_dis.deployer.as_ref().unwrap()[2..].to_lowercase(), &to_dis.deployer.as_ref().unwrap()[2..].to_lowercase()) ; 
        }
   }
    
    Some(ret_str)
}

#[cfg(test)] 
mod test {  
    use crate::deploy::transaction::get_transaction_data;
    use crate::deploy::registry::RainNetworks;
    use crate::deploy::registry::Mumbai;
    use crate::deploy::registry::Fuji;
    use crate::deploy::dis::DISpair;
    use super::replace_dis_pair;
    use std::env ;



    #[tokio::test]
    async fn test_replace_no_dis() { 

        let tx_hash = String::from("0xea76ed73832498c4293aa06aeca2899f2b5adca15d703b03690185ed829f3e71") ;  
        // Reading environment variables
        let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
        let network: RainNetworks = RainNetworks::Mumbai(mumbai_network);  

        let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ; 

        let from_dis = DISpair {
            interpreter : None ,
            store : None ,  
            deployer : None
        } ; 

        let to_dis = DISpair {
            interpreter : None ,
            store : None , 
            deployer : None
        } ; 

        let replaced_data = replace_dis_pair(
            tx_data.clone(),
            from_dis,
            to_dis
        ).unwrap() ;

        assert_eq!(tx_data, replaced_data);
    }

    #[tokio::test]
   async fn test_replace_only_from_dis() { 

        let tx_hash = String::from("0xc215bf3dc7440687ca20e028158e58640eeaec72d6fe6738f6d07843835c2cde") ;  
        // Reading environment variables
        let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
        let network: RainNetworks = RainNetworks::Mumbai(mumbai_network) ;   
        let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ; 

        let from_dis = DISpair {
            interpreter : Some(String::from("0x5f02c2f831d3e0d430aa58c973b8b751f3d81b38")),
            store : Some(String::from("0xa5d9c16ddfd05d398fd0f302edd9e9e16d328796")),  
            deployer : Some(String::from("0xd3870063bcf25d5110ab9df9672a0d5c79c8b2d5")),
        } ; 

        let to_dis = DISpair {
            interpreter : None ,
            store : None , 
            deployer : None
        } ; 

        let replaced_data = replace_dis_pair(
            tx_data.clone(),
            from_dis,
            to_dis
        ).unwrap() ;

        assert_eq!(tx_data, replaced_data);
    }

    #[tokio::test]
    async fn test_replace_only_to_dis() { 

        let tx_hash = String::from("0xc215bf3dc7440687ca20e028158e58640eeaec72d6fe6738f6d07843835c2cde") ;  
        // Reading environment variables
        let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
        let network: RainNetworks = RainNetworks::Mumbai(mumbai_network) ;  
        let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ; 

        let from_dis = DISpair {
            interpreter : None ,
            store : None , 
            deployer : None
        } ; 

        let to_dis = DISpair {
            interpreter : Some(String::from("0xfd1da7eee4a9391f6fcabb28617f41894ba84cdc")),
            store : Some(String::from("0x9b8571bd2742ec628211111de3aa940f5984e82b")),  
            deployer : Some(String::from("0x3d7d894afc7dbfd45bf50867c9b051da8eee85e9")),
        } ; 

        let replaced_data = replace_dis_pair(
            tx_data.clone(),
            from_dis,
            to_dis
        ).unwrap() ;

        assert_eq!(tx_data, replaced_data);
    }

    #[tokio::test]
    async fn test_replace_from_to_dis() { 

        let tx_hash = String::from("0xebacdb3971924c9bbd2257334d436b4590d3d98f54969f6f942d6bd7a68da80b") ;   

        // Reading environment variables
        let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
        let network: RainNetworks = RainNetworks::Mumbai(mumbai_network) ;    
        let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ;  

        // Reading environment variables
        let fuji_network = Fuji::new(env::var("FUJI_RPC_URL").unwrap(), env::var("SNOWTRACE_API_KEY").unwrap()) ; 
        let to_network: RainNetworks = RainNetworks::Fuji(fuji_network) ;  
        let expexted_tx_hash = String::from("0xb0ae6ff12e9b810530e1b0844a448865cf4781950a90c99ba36f7f343e596717") ;   
        let expected_tx_data = get_transaction_data(to_network, expexted_tx_hash).await.unwrap() ;  


        let from_dis = DISpair {
            interpreter : Some(String::from("0x5f02c2f831d3e0d430aa58c973b8b751f3d81b38")) ,
            store : Some(String::from("0xa5d9c16ddfd05d398fd0f302edd9e9e16d328796")) , 
            deployer : None
        } ; 

        let to_dis = DISpair {
            interpreter : Some(String::from("0xfd1da7eee4a9391f6fcabb28617f41894ba84cdc")),
            store : Some(String::from("0x9b8571bd2742ec628211111de3aa940f5984e82b")),  
            deployer : None,
        } ; 

        let replaced_data = replace_dis_pair(
            tx_data,
            from_dis,
            to_dis
        ).unwrap() ;

        assert_eq!(expected_tx_data, replaced_data);
    } 

    #[tokio::test]
    async fn test_replace_dis() { 

        let tx_hash = String::from("0xc215bf3dc7440687ca20e028158e58640eeaec72d6fe6738f6d07843835c2cde") ;   
        // Reading environment variables
        let mumbai_network = Mumbai::new(env::var("MUMBAI_RPC_URL").unwrap(), env::var("POLYGONSCAN_API_KEY").unwrap()) ; 
        let network: RainNetworks = RainNetworks::Mumbai(mumbai_network) ;    
        let tx_data = get_transaction_data(network, tx_hash).await.unwrap() ;  

        // Reading environment variables
        let fuji_network = Fuji::new(env::var("FUJI_RPC_URL").unwrap(), env::var("SNOWTRACE_API_KEY").unwrap()) ; 
        let to_network: RainNetworks = RainNetworks::Fuji(fuji_network);
        let expexted_tx_hash = String::from("0x13b9895c7eb7311bbb22ef0a692b7b115c98c957514903e7c3a0e454e3389378") ;   
        let expected_tx_data = get_transaction_data(to_network, expexted_tx_hash).await.unwrap() ;  


        let from_dis = DISpair {
            interpreter : Some(String::from("0x5f02c2f831d3e0d430aa58c973b8b751f3d81b38")) ,
            store : Some(String::from("0xa5d9c16ddfd05d398fd0f302edd9e9e16d328796")) , 
            deployer : Some(String::from("0xd3870063bcf25d5110ab9df9672a0d5c79c8b2d5"))
        } ; 

        let to_dis = DISpair {
            interpreter : Some(String::from("0xfd1da7eee4a9391f6fcabb28617f41894ba84cdc")),
            store : Some(String::from("0x9b8571bd2742ec628211111de3aa940f5984e82b")),  
            deployer : Some(String::from("0x3d7d894afc7dbfd45bf50867c9b051da8eee85e9")),
        } ; 

        let replaced_data = replace_dis_pair(
            tx_data,
            from_dis,
            to_dis
        ).unwrap() ;

        assert_eq!(expected_tx_data, replaced_data);
    } 
}