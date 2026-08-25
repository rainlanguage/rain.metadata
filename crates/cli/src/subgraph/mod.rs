use crate::error::Error;

/// All known subgraph endpoints
#[derive(Debug, Clone)]
pub struct KnownSubgraphs;

impl KnownSubgraphs {
    /// Rain known subgraphs on ethereum mainnet
    pub const ETHEREUM: [&'static str; 3] = [
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-ethereum", // legacy endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np-eth", // np endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2-eth", // npe2 endpoint
    ];

    /// Rain known subgraphs on polygon mainnet
    pub const POLYGON: [&'static str; 3] = [
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-polygon", // legacy endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np-matic", // np endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2-mati", // npe2 endpoint
    ];

    /// Rain known subgraphs on mumbai (polygon testnet)
    pub const MUMBAI: [&'static str; 3] = [
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry", // legacy endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np", // np endpoint
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2", // npe2 endpoint
    ];

    /// Rain NPE2 subgraphs of all supported networks
    pub const NPE2: [&'static str; 3] = [Self::ETHEREUM[2], Self::POLYGON[2], Self::MUMBAI[2]];

    /// Rain NativeParser subgraphs of all supported networks
    pub const NP: [&'static str; 3] = [Self::ETHEREUM[1], Self::POLYGON[1], Self::MUMBAI[1]];

    /// Rain legacy(non NativeParser) subgraphs of all supported networks
    pub const LEGACY: [&'static str; 3] = [Self::ETHEREUM[0], Self::POLYGON[0], Self::MUMBAI[0]];

    /// All Rain known subgraph endpoint URLs
    pub const ALL: [&'static str; 9] = [
        Self::ETHEREUM[0],
        Self::ETHEREUM[1],
        Self::ETHEREUM[2],
        Self::POLYGON[0],
        Self::POLYGON[1],
        Self::POLYGON[2],
        Self::MUMBAI[0],
        Self::MUMBAI[1],
        Self::MUMBAI[2],
    ];

    /// get the subgraph endpoint from a chain id
    pub fn of_chain(chain_id: u64) -> Result<[&'static str; 3], Error> {
        match chain_id {
            1 => Ok(Self::ETHEREUM),
            137 => Ok(Self::POLYGON),
            80001 => Ok(Self::MUMBAI),
            _ => Err(Error::UnsupportedNetwork),
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    const ETH_LEGACY: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-ethereum";
    const ETH_NP: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np-eth";
    const ETH_NPE2: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2-eth";
    const POLY_LEGACY: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-polygon";
    const POLY_NP: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np-matic";
    const POLY_NPE2: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2-mati";
    const MUMBAI_LEGACY: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry";
    const MUMBAI_NP: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-np";
    const MUMBAI_NPE2: &str =
        "https://api.thegraph.com/subgraphs/name/rainlanguage/interpreter-registry-npe2";

    /// The per-network triples are [legacy, np, npe2] with exactly these
    /// URLs.
    #[test]
    fn test_network_triples_are_exact() {
        assert_eq!(KnownSubgraphs::ETHEREUM, [ETH_LEGACY, ETH_NP, ETH_NPE2]);
        assert_eq!(KnownSubgraphs::POLYGON, [POLY_LEGACY, POLY_NP, POLY_NPE2]);
        assert_eq!(
            KnownSubgraphs::MUMBAI,
            [MUMBAI_LEGACY, MUMBAI_NP, MUMBAI_NPE2]
        );
    }

    /// The flavor slices pick the same column from every network, and ALL
    /// concatenates the three networks in order.
    #[test]
    fn test_flavor_slices_and_all() {
        assert_eq!(
            KnownSubgraphs::LEGACY,
            [ETH_LEGACY, POLY_LEGACY, MUMBAI_LEGACY]
        );
        assert_eq!(KnownSubgraphs::NP, [ETH_NP, POLY_NP, MUMBAI_NP]);
        assert_eq!(KnownSubgraphs::NPE2, [ETH_NPE2, POLY_NPE2, MUMBAI_NPE2]);
        assert_eq!(
            KnownSubgraphs::ALL,
            [
                ETH_LEGACY,
                ETH_NP,
                ETH_NPE2,
                POLY_LEGACY,
                POLY_NP,
                POLY_NPE2,
                MUMBAI_LEGACY,
                MUMBAI_NP,
                MUMBAI_NPE2,
            ]
        );
    }

    /// of_chain maps 1/137/80001 to their networks.
    #[test]
    fn test_of_chain_known_networks() {
        assert_eq!(
            KnownSubgraphs::of_chain(1).unwrap(),
            KnownSubgraphs::ETHEREUM
        );
        assert_eq!(
            KnownSubgraphs::of_chain(137).unwrap(),
            KnownSubgraphs::POLYGON
        );
        assert_eq!(
            KnownSubgraphs::of_chain(80001).unwrap(),
            KnownSubgraphs::MUMBAI
        );
    }

    /// Every other chain id is unsupported.
    #[test]
    fn test_of_chain_unknown_network_errors() {
        for id in [0u64, 2, 100, 8453, u64::MAX] {
            assert!(matches!(
                KnownSubgraphs::of_chain(id),
                Err(Error::UnsupportedNetwork)
            ));
        }
    }
}
