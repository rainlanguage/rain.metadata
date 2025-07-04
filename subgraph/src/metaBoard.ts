import { BigInt, crypto, Bytes } from "@graphprotocol/graph-ts";
import { MetaV1_2 as MetaV1Event } from "../generated/metaboard0/MetaBoard";
import { MetaBoard, MetaV1 } from "../generated/schema";
import { createTransactionEntity } from "./transaction";

export function handleMetaV1_2(event: MetaV1Event): void {
  createTransactionEntity(event);
  let metaBoard = MetaBoard.load(event.address);
  if ( !metaBoard ) {
    metaBoard = new MetaBoard(event.address);
    metaBoard.address = event.address;
    metaBoard.nextMetaId = BigInt.fromI32(0);
    metaBoard.save();
  }

  let metaV1 = new MetaV1(metaBoard.nextMetaId.toString());
  metaV1.transaction = event.transaction.hash;

  metaV1.metaBoard = metaBoard.address;

  metaV1.sender = event.params.sender;
  metaV1.subject = event.params.subject;

  metaV1.metaHash = Bytes.fromByteArray(crypto.keccak256(event.params.meta));

  metaV1.meta = event.params.meta;

  metaV1.save();

  metaBoard.nextMetaId = metaBoard.nextMetaId.plus(BigInt.fromI32(1));
  metaBoard.save();
}