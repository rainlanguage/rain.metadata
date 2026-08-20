// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std-1.16.1/src/Test.sol";
import {LibRainDeploy} from "rain-deploy-0.1.3/src/lib/LibRainDeploy.sol";
import {LibMetaBoardDeploy} from "src/lib/deploy/LibMetaBoardDeploy.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

contract LibMetaBoardDeployTest is Test {
    function testDeployAddress() external {
        // Ethereum mainnet has the zoltu factory deployed but no MetaBoard,
        // so the deterministic deploy cannot collide with a live deployment.
        // ETHEREUM_RPC_URL is exported by the rainix rpc-preflight in CI.
        vm.createSelectFork(vm.envString("ETHEREUM_RPC_URL"));

        address deployedAddress = LibRainDeploy.deployZoltu(type(MetaBoard).creationCode);

        assertEq(deployedAddress, LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS);
        assertTrue(address(deployedAddress).code.length > 0, "Deployed address has no code");

        assertEq(address(deployedAddress).codehash, LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH);
    }

    function testExpectedCodeHash() external {
        MetaBoard metaBoard = new MetaBoard();

        assertEq(address(metaBoard).codehash, LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH);
    }

    function checkProdDeployment(string memory envVar) internal {
        vm.createSelectFork(vm.envString(envVar));
        address deployed = LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS;
        assertTrue(deployed.code.length > 0, string.concat("MetaBoard not deployed: ", envVar));
        assertEq(
            deployed.codehash,
            LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH,
            string.concat("MetaBoard codehash mismatch: ", envVar)
        );
    }

    function testProdDeployArbitrum() external {
        checkProdDeployment("ARBITRUM_RPC_URL");
    }

    function testProdDeployBase() external {
        checkProdDeployment("BASE_RPC_URL");
    }

    function testProdDeployBaseSepolia() external {
        checkProdDeployment("BASE_SEPOLIA_RPC_URL");
    }

    function testProdDeployFlare() external {
        checkProdDeployment("FLARE_RPC_URL");
    }

    function testProdDeployPolygon() external {
        checkProdDeployment("POLYGON_RPC_URL");
    }

    /// Start block DISCOVERY (`LibRainDeploy.findDeployBlock`) is deliberately
    /// not tested here: its rollFork binary search hits RPC rate limits, and on
    /// Arbitrum rollFork maps to L1 block numbers, not L2 (the Arbitrum start
    /// block was found via manual binary search using eth_getCode RPC calls
    /// against L2 block numbers). The `testIsStartBlock*` tests below verify
    /// the pinned constants directly and cheaply instead.
    function checkIsStartBlock(string memory rpcEnvVar, uint256 startBlock) internal {
        vm.createSelectFork(vm.envString(rpcEnvVar));
        assertTrue(
            LibRainDeploy.isStartBlock(
                vm,
                LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS,
                LibMetaBoardDeploy.METABOARD_DEPLOYED_CODEHASH,
                startBlock
            ),
            string.concat("not start block: ", rpcEnvVar)
        );
    }

    function testIsStartBlockArbitrum() external {
        checkIsStartBlock("ARBITRUM_RPC_URL", LibMetaBoardDeploy.METABOARD_START_BLOCK_ARBITRUM);
    }

    function testIsStartBlockBase() external {
        checkIsStartBlock("BASE_RPC_URL", LibMetaBoardDeploy.METABOARD_START_BLOCK_BASE);
    }

    function testIsStartBlockBaseSepolia() external {
        checkIsStartBlock("BASE_SEPOLIA_RPC_URL", LibMetaBoardDeploy.METABOARD_START_BLOCK_BASE_SEPOLIA);
    }

    function testIsStartBlockFlare() external {
        checkIsStartBlock("FLARE_RPC_URL", LibMetaBoardDeploy.METABOARD_START_BLOCK_FLARE);
    }

    function testIsStartBlockPolygon() external {
        checkIsStartBlock("POLYGON_RPC_URL", LibMetaBoardDeploy.METABOARD_START_BLOCK_POLYGON);
    }

    function testSubgraphYamlAddress() external {
        string[] memory inputs = new string[](3);
        inputs[0] = "yq";
        inputs[1] = ".dataSources[0].source.address";
        inputs[2] = "subgraph/subgraph.yaml";
        bytes memory result = vm.ffi(inputs);
        address addr = address(bytes20(result));
        assertEq(addr, LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS, "subgraph.yaml address mismatch");
    }

    function testSubgraphTestAddressTs() external {
        string[] memory inputs = new string[](4);
        inputs[0] = "grep";
        inputs[1] = "-oP";
        inputs[2] = "0x[0-9a-fA-F]{40}";
        inputs[3] = "subgraph/tests/address.ts";
        bytes memory result = vm.ffi(inputs);
        address addr = address(bytes20(result));
        assertEq(addr, LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS, "subgraph/tests/address.ts address mismatch");
    }

    function testNetworksJsonAddresses() external view {
        string memory json = vm.readFile("subgraph/networks.json");
        string[] memory networks = vm.parseJsonKeys(json, "$");
        for (uint256 i = 0; i < networks.length; i++) {
            string memory path = string.concat(".", networks[i], ".metaboard0.address");
            address addr = vm.parseJsonAddress(json, path);
            assertEq(
                addr,
                LibMetaBoardDeploy.METABOARD_DEPLOYED_ADDRESS,
                string.concat("networks.json address mismatch: ", networks[i])
            );
        }
    }

    function checkNetworksJsonStartBlock(string memory networkKey, uint256 expectedStartBlock) internal view {
        string memory json = vm.readFile("subgraph/networks.json");
        string memory path = string.concat(".", networkKey, ".metaboard0.startBlock");
        uint256 startBlock = vm.parseJsonUint(json, path);
        assertEq(startBlock, expectedStartBlock, string.concat("networks.json startBlock mismatch: ", networkKey));
    }

    function testNetworksJsonStartBlockArbitrum() external view {
        checkNetworksJsonStartBlock("arbitrum-one", LibMetaBoardDeploy.METABOARD_START_BLOCK_ARBITRUM);
    }

    function testNetworksJsonStartBlockBase() external view {
        checkNetworksJsonStartBlock("base", LibMetaBoardDeploy.METABOARD_START_BLOCK_BASE);
    }

    function testNetworksJsonStartBlockBaseSepolia() external view {
        checkNetworksJsonStartBlock("base-sepolia", LibMetaBoardDeploy.METABOARD_START_BLOCK_BASE_SEPOLIA);
    }

    function testNetworksJsonStartBlockFlare() external view {
        checkNetworksJsonStartBlock("flare", LibMetaBoardDeploy.METABOARD_START_BLOCK_FLARE);
    }

    function testNetworksJsonStartBlockPolygon() external view {
        checkNetworksJsonStartBlock("matic", LibMetaBoardDeploy.METABOARD_START_BLOCK_POLYGON);
    }
}
