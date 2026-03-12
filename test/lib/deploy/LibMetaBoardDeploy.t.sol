// SPDX-License-Identifier: LicenseRef-DCL-1.0
// SPDX-FileCopyrightText: Copyright (c) 2020 Rain Open Source Software Ltd
pragma solidity =0.8.25;

import {Test} from "forge-std/Test.sol";
import {LibRainDeploy} from "rain.deploy/lib/LibRainDeploy.sol";
import {LibMetaBoardDeploy} from "src/lib/deploy/LibMetaBoardDeploy.sol";
import {MetaBoard} from "src/concrete/MetaBoard.sol";

contract LibMetaBoardDeployTest is Test {
    function testDeployAddress() external {
        vm.createSelectFork(vm.envString("CI_FORK_ETH_RPC_URL"));

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
        checkProdDeployment("CI_FORK_ARB_RPC_URL");
    }

    function testProdDeployBase() external {
        checkProdDeployment("CI_FORK_BASE_RPC_URL");
    }

    function testProdDeployBaseSepolia() external {
        checkProdDeployment("CI_FORK_BASE_SEPOLIA_RPC_URL");
    }

    function testProdDeployFlare() external {
        checkProdDeployment("CI_FORK_FLARE_RPC_URL");
    }

    function testProdDeployPolygon() external {
        checkProdDeployment("CI_FORK_POLYGON_RPC_URL");
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
}
