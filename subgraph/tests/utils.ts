import { MetaV1_2 } from "../generated/metaboard0/MetaBoard"; // Update the path as per your file structure
import { ethereum, Address, BigInt, Bytes } from "@graphprotocol/graph-ts";
import { newMockEvent } from "matchstick-as";
import { handleMetaV1_2 } from "../src/metaBoard";


export function createNewMetaV1Event(sender: string, subject: Bytes, meta: Bytes): MetaV1_2 {
  // Create a mock ethereum.Event instance
  const metaV1Event = changetype<MetaV1_2>(newMockEvent());
  metaV1Event.parameters = new Array();
  metaV1Event.address = CONTRACT_ADDRESS;

  // Set up transaction data
  metaV1Event.transaction.hash = Bytes.fromHexString("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef");
  metaV1Event.transaction.from = Address.fromString(sender);
  metaV1Event.block.number = BigInt.fromI32(1);
  metaV1Event.block.timestamp = BigInt.fromI32(1000);

  let senderParam = new ethereum.EventParam("sender", ethereum.Value.fromAddress(Address.fromString(sender)));
  let subjectParam = new ethereum.EventParam("subject", ethereum.Value.fromBytes(subject));
  let metaParam = new ethereum.EventParam("meta", ethereum.Value.fromBytes(meta));

  metaV1Event.parameters.push(senderParam);
  metaV1Event.parameters.push(subjectParam);
  metaV1Event.parameters.push(metaParam);
  return metaV1Event;
}

export function handleNewMetaV1Events(events: MetaV1_2[]): void {
  events.forEach(event => {
    handleMetaV1_2(event);
  });
}

export const CONTRACT_ADDRESS = Address.fromString("0x23F77e7Bc935503e437166498D7D72f2Ea290E1f");
