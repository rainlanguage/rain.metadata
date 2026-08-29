import {
  test,
  assert,
  clearStore,
  describe,
  afterEach,
  beforeAll,
  afterAll,
  newMockEvent,
  clearInBlockStore,
} from "matchstick-as";
import {
  createNewMetaV1Event,
  handleNewMetaV1Events,
  CONTRACT_ADDRESS,
  OTHER_CONTRACT_ADDRESS,
} from "./utils";
import { Bytes, BigInt, ethereum, Address } from "@graphprotocol/graph-ts";
import {
  MetaV1_2,
} from "../generated/metaboard0/MetaBoard";
import {
  MetaBoard,
  MetaV1 as MetaV1Entity,
  Transaction,
} from "../generated/schema";
import { handleMetaV1_2 } from "../src/metaBoard";
import { createTransactionEntity } from "../src/transaction";

const ENTITY_TYPE_META_V1 = "MetaV1";
const ENTITY_TYPE_META_BOARD = "MetaBoard";
const ENTITY_TYPE_TRANSACTION = "Transaction";
const sender = "0xc0D477556c25C9d67E1f57245C7453DA776B51cf";
const subject = Bytes.fromHexString(
  "0x3299321d9db6e1dc95c371c5aea791e7c45c4b1b1d4ff713664e6d2187ab7aa5",
);
const metaString = "0xff0a89c674ee7874010203";
const metaHashString =
  "0x6bdf81f785b54fd65ca6fc5d02b40fa361bc7d5f4f1067fc534b9433ecbc784d";
const transactionHash =
  "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
const transactionBlockNumber = 32377304;
const transactionTimestamp = 1751543962;
const otherMetaString = "0xff0a89c674ee7874040506";
const otherMetaHashString =
  "0xc9d578b9ed6efc27f27b866fa548a98b91787a8b5c3e26b2fcc5763388a17079";
const otherTransactionHash =
  "0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321";
// Spelled out rather than built the way `handleMetaV1_2` builds them, so the
// id scheme is pinned to something outside the mapping.
const FIRST_BOARD_META_ID = Bytes.fromHexString(
  "0xfb8437aefbb8031064e274527c5fc08e30ac69280000000000000000000000000000000000000000000000000000000000000000",
);
const FIRST_BOARD_SECOND_META_ID = Bytes.fromHexString(
  "0xfb8437aefbb8031064e274527c5fc08e30ac69280000000000000000000000000000000000000000000000000000000000000001",
);
const OTHER_BOARD_META_ID = Bytes.fromHexString(
  "0x4a9b0f6c1e3d2a5b8c7d6e9f0a1b2c3d4e5f6a7b0000000000000000000000000000000000000000000000000000000000000000",
);

describe("Test meta event", () => {
  afterEach(() => {
    clearStore();
    clearInBlockStore();
  });
  test("Checks event params", () => {
    // Call mappings
    const meta = Bytes.fromHexString(metaString);

    const newMetaV1Event = createNewMetaV1Event(
      CONTRACT_ADDRESS,
      sender,
      subject,
      meta,
      transactionHash,
      transactionBlockNumber,
      transactionTimestamp,
    );

    handleMetaV1_2(newMetaV1Event);

    assert.entityCount(ENTITY_TYPE_META_V1, 1);
    assert.addressEquals(newMetaV1Event.address, CONTRACT_ADDRESS);
    assert.equals(
      ethereum.Value.fromBytes(newMetaV1Event.params.subject),
      ethereum.Value.fromBytes(subject),
    );
    assert.equals(
      ethereum.Value.fromBytes(newMetaV1Event.params.meta),
      ethereum.Value.fromBytes(meta),
    );
  });
  test("Can update event metadata", () => {
    const metaV1Event = changetype<MetaV1_2>(newMockEvent());
    metaV1Event.parameters = new Array();

    const subject = Bytes.fromHexString(
      "0xe61c27d16fa0dfbb69b2e8c1a1beb64051668e348f4bb52e843548759b8fabe1",
    );
    const meta = Bytes.fromHexString(metaString);

    let UPDATED_SENDER = new ethereum.EventParam(
      "sender",
      ethereum.Value.fromAddress(Address.fromString(sender)),
    );
    let UPDATED_SUBJECT = new ethereum.EventParam(
      "subject",
      ethereum.Value.fromBytes(subject),
    );
    let UPDATED_META = new ethereum.EventParam(
      "meta",
      ethereum.Value.fromBytes(meta),
    );

    metaV1Event.parameters.push(UPDATED_SENDER);
    metaV1Event.parameters.push(UPDATED_SUBJECT);
    metaV1Event.parameters.push(UPDATED_META);

    assert.addressEquals(Address.fromString(sender), metaV1Event.params.sender);
    assert.bytesEquals(subject, metaV1Event.params.subject);
    assert.bytesEquals(meta, metaV1Event.params.meta);
  });
  test("Returns null when calling entity.load() if an entity doesn't exist", () => {
    let retrievedMetaV1 = MetaV1Entity.load(FIRST_BOARD_META_ID);
    assert.assertNull(retrievedMetaV1);
  });

  test("Can create transaction entity directly", () => {
    const metaV1Event = changetype<MetaV1_2>(newMockEvent());
    metaV1Event.parameters = new Array();
    metaV1Event.address = CONTRACT_ADDRESS;

    // Set up transaction data
    metaV1Event.transaction.hash = Bytes.fromHexString(transactionHash);
    metaV1Event.transaction.from = Address.fromString(sender);
    metaV1Event.block.number = BigInt.fromI32(transactionBlockNumber);
    metaV1Event.block.timestamp = BigInt.fromI32(transactionTimestamp);

    // Call createTransactionEntity directly
    const transactionId = createTransactionEntity(metaV1Event);

    // Verify transaction was created
    const retrievedTransaction = Transaction.load(transactionId) as Transaction;
    assert.entityCount(ENTITY_TYPE_TRANSACTION, 1);
    assert.bytesEquals(
      retrievedTransaction.id,
      Bytes.fromHexString(transactionHash),
    );
    assert.bigIntEquals(
      retrievedTransaction.blockNumber,
      BigInt.fromString(transactionBlockNumber.toString()),
    );
    assert.bigIntEquals(
      retrievedTransaction.timestamp,
      BigInt.fromString(transactionTimestamp.toString()),
    );
    assert.bytesEquals(retrievedTransaction.from, Address.fromString(sender));
  });

  test("Create transaction entity returns existing transaction if already exists", () => {
    const metaV1Event = changetype<MetaV1_2>(newMockEvent());
    metaV1Event.parameters = new Array();
    metaV1Event.address = CONTRACT_ADDRESS;

    // Set up transaction data
    metaV1Event.transaction.hash = Bytes.fromHexString(transactionHash);
    metaV1Event.transaction.from = Address.fromString(sender);
    metaV1Event.block.number = BigInt.fromI32(transactionBlockNumber);
    metaV1Event.block.timestamp = BigInt.fromI32(transactionTimestamp);

    // Call createTransactionEntity twice
    const transactionId1 = createTransactionEntity(metaV1Event);
    const transactionId2 = createTransactionEntity(metaV1Event);

    // Verify both calls return the same transaction ID
    assert.bytesEquals(transactionId1, transactionId2);

    // Verify only one transaction entity exists
    assert.entityCount(ENTITY_TYPE_TRANSACTION, 1);

    // Verify the transaction has the correct data
    const retrievedTransaction = Transaction.load(
      transactionId1,
    ) as Transaction;
    assert.bytesEquals(
      retrievedTransaction.id,
      Bytes.fromHexString(transactionHash),
    );
    assert.bigIntEquals(
      retrievedTransaction.blockNumber,
      BigInt.fromString(transactionBlockNumber.toString()),
    );
    assert.bigIntEquals(
      retrievedTransaction.timestamp,
      BigInt.fromString(transactionTimestamp.toString()),
    );
    assert.bytesEquals(retrievedTransaction.from, Address.fromString(sender));
  });
});

describe("Test MetaBoard and MetaV1 Entities", () => {
  beforeAll(() => {
    const meta = Bytes.fromHexString(metaString);
    const newMetaV1Event = createNewMetaV1Event(
      CONTRACT_ADDRESS,
      sender,
      subject,
      meta,
      transactionHash,
      transactionBlockNumber,
      transactionTimestamp,
    );

    handleMetaV1_2(newMetaV1Event);
  });

  afterAll(() => {
    clearStore();
    clearInBlockStore();
  });

  test("Checks MetaBoard entity", () => {
    let retrievedMetaBoard = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    assert.entityCount(ENTITY_TYPE_META_BOARD, 1);
    assert.addressEquals(
      Address.fromBytes(retrievedMetaBoard.address),
      CONTRACT_ADDRESS,
    );
  });

  test("Returns null when calling entity.loadInBlock() if an entity doesn't exist in the current block", () => {
    let retrievedMetaBoard = MetaBoard.loadInBlock(
      Address.fromString("0x33F77e7Bc935503e437166498D7D72f2Ea290E1f"),
    );
    assert.assertNull(retrievedMetaBoard);
  });

  test("Checks MetaBoard entity id", () => {
    let retrievedMetaBoard = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    assert.entityCount(ENTITY_TYPE_META_BOARD, 1);
    assert.bytesEquals(retrievedMetaBoard.id, CONTRACT_ADDRESS);
  });

  test("Checks MetaV1 entity data", () => {
    let retrievedMetaV1 = MetaV1Entity.load(
      FIRST_BOARD_META_ID,
    ) as MetaV1Entity;
    assert.entityCount(ENTITY_TYPE_META_V1, 1);
    assert.addressEquals(
      Address.fromBytes(retrievedMetaV1.sender),
      Address.fromString(sender),
    ); //sender
    assert.bytesEquals(retrievedMetaV1.subject, subject); //subject
    assert.bytesEquals(retrievedMetaV1.metaBoard, CONTRACT_ADDRESS); //metaBoard
    assert.bytesEquals(retrievedMetaV1.meta, Bytes.fromHexString(metaString)); //meta
    assert.bytesEquals(
      retrievedMetaV1.metaHash,
      Bytes.fromHexString(metaHashString),
    ); //metaHash
    assert.bytesEquals(
      retrievedMetaV1.transaction,
      Bytes.fromHexString(transactionHash),
    ); //transaction
  });

  test("Checks Transaction entity is created", () => {
    const retrievedTransaction = Transaction.load(
      Bytes.fromHexString(transactionHash),
    ) as Transaction;
    assert.entityCount(ENTITY_TYPE_TRANSACTION, 1);
    assert.bytesEquals(
      retrievedTransaction.id,
      Bytes.fromHexString(transactionHash),
    );
    assert.bigIntEquals(
      retrievedTransaction.blockNumber,
      BigInt.fromString(transactionBlockNumber.toString()),
    );
    assert.bigIntEquals(
      retrievedTransaction.timestamp,
      BigInt.fromString(transactionTimestamp.toString()),
    );
    assert.bytesEquals(retrievedTransaction.from, Address.fromString(sender));
  });
});

// `nextMetaId` is counted per MetaBoard entity, so every board counts from
// zero. While the MetaV1 id was that counter alone, a deployment indexing two
// boards gave both boards' first meta the id "0" and the store overwrote the
// first with the second: a meta that had been indexed, and its metaHash, left
// the index. IDescribedByMetaV1 implies an indexer that can retrieve the
// metadata for a given hash, so that loss is the interface's own claim
// failing. rainlanguage/rain.metadata#206.
describe("Test MetaV1 ids are scoped to their metaboard", () => {
  afterEach(() => {
    clearStore();
    clearInBlockStore();
  });

  test("Two metaboards do not overwrite each other's metas", () => {
    handleMetaV1_2(
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        transactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    );
    handleMetaV1_2(
      createNewMetaV1Event(
        OTHER_CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(otherMetaString),
        otherTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    );

    assert.entityCount(ENTITY_TYPE_META_BOARD, 2);
    assert.entityCount(ENTITY_TYPE_META_V1, 2);

    const firstBoardMeta = MetaV1Entity.load(FIRST_BOARD_META_ID);
    assert.assertNotNull(firstBoardMeta);
    assert.bytesEquals(
      (firstBoardMeta as MetaV1Entity).metaHash,
      Bytes.fromHexString(metaHashString),
    );
    assert.bytesEquals(
      (firstBoardMeta as MetaV1Entity).metaBoard,
      CONTRACT_ADDRESS,
    );

    const otherBoardMeta = MetaV1Entity.load(OTHER_BOARD_META_ID);
    assert.assertNotNull(otherBoardMeta);
    assert.bytesEquals(
      (otherBoardMeta as MetaV1Entity).metaHash,
      Bytes.fromHexString(otherMetaHashString),
    );
    assert.bytesEquals(
      (otherBoardMeta as MetaV1Entity).metaBoard,
      OTHER_CONTRACT_ADDRESS,
    );

    const firstBoard = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    const otherBoard = MetaBoard.load(OTHER_CONTRACT_ADDRESS) as MetaBoard;
    assert.bigIntEquals(firstBoard.nextMetaId, BigInt.fromI32(1));
    assert.bigIntEquals(otherBoard.nextMetaId, BigInt.fromI32(1));
  });

  test("One metaboard's metas do not overwrite each other", () => {
    handleMetaV1_2(
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        transactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    );
    handleMetaV1_2(
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(otherMetaString),
        otherTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    );

    assert.entityCount(ENTITY_TYPE_META_BOARD, 1);
    assert.entityCount(ENTITY_TYPE_META_V1, 2);

    const firstMeta = MetaV1Entity.load(FIRST_BOARD_META_ID);
    assert.assertNotNull(firstMeta);
    assert.bytesEquals(
      (firstMeta as MetaV1Entity).metaHash,
      Bytes.fromHexString(metaHashString),
    );

    const secondMeta = MetaV1Entity.load(FIRST_BOARD_SECOND_META_ID);
    assert.assertNotNull(secondMeta);
    assert.bytesEquals(
      (secondMeta as MetaV1Entity).metaHash,
      Bytes.fromHexString(otherMetaHashString),
    );

    const board = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    assert.bigIntEquals(board.nextMetaId, BigInt.fromI32(2));
  });
});

function metaPayload(index: i32): Bytes {
  return Bytes.fromHexString(metaString + index.toString(16).padStart(2, "0"));
}

function metaWithPayload(metas: MetaV1Entity[], payload: Bytes): MetaV1Entity {
  for (let i = 0; i < metas.length; i++) {
    if (metas[i].meta == payload) {
      return metas[i];
    }
  }
  throw new Error("no meta carrying " + payload.toHexString());
}

// The MetaV1 id ends in the board's `nextMetaId` counter, and `orderBy: id`
// sorts ids, not counters. While that counter was a decimal string a board's
// eleventh meta sorted before its third, so a consumer paginating by id read a
// board's metas out of the order they were emitted in.
// rainlanguage/rain.metadata#227.
describe("Test MetaV1 ids sort chronologically", () => {
  afterEach(() => {
    clearStore();
    clearInBlockStore();
  });

  test("A metaboard's eleventh meta sorts after its third", () => {
    for (let i = 0; i < 11; i++) {
      handleMetaV1_2(
        createNewMetaV1Event(
          CONTRACT_ADDRESS,
          sender,
          subject,
          metaPayload(i),
          transactionHash,
          transactionBlockNumber,
          transactionTimestamp,
        ),
      );
    }

    const metas = (MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard).metas.load();
    assert.i32Equals(metas.length, 11);

    const third = metaWithPayload(metas, metaPayload(2));
    const eleventh = metaWithPayload(metas, metaPayload(10));
    assert.assertTrue(third.id.toHexString() < eleventh.id.toHexString());
  });
});

// `metas: [MetaV1!] @derivedFrom(field: "metaBoard")` resolves
// `metaV1.metaBoard` against MetaBoard ids, so that field has to carry the
// board's id. The handler writes a board's `id` and its `address` from the same
// `event.address`, so a board whose two differ is the only store state that
// tells the two fields apart. rainlanguage/rain.metadata#227.
describe("Test MetaV1 metaBoard relation", () => {
  afterEach(() => {
    clearStore();
    clearInBlockStore();
  });

  test("The relation is the metaboard's id, not its address field", () => {
    const board = new MetaBoard(CONTRACT_ADDRESS);
    board.address = OTHER_CONTRACT_ADDRESS;
    board.nextMetaId = BigInt.fromI32(0);
    board.save();

    handleMetaV1_2(
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        transactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    );

    const metas = (MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard).metas.load();
    assert.i32Equals(metas.length, 1);
    assert.bytesEquals(metas[0].id, FIRST_BOARD_META_ID);
    assert.bytesEquals(metas[0].metaBoard, CONTRACT_ADDRESS);
  });
});

const firstCounterTransactionHash =
  "0x1111111111111111111111111111111111111111111111111111111111111111";
const secondCounterTransactionHash =
  "0x2222222222222222222222222222222222222222222222222222222222222222";
const thirdCounterTransactionHash =
  "0x3333333333333333333333333333333333333333333333333333333333333333";

describe("Test MetaBoard nextMetaId counter", () => {
  afterEach(() => {
    clearStore();
    clearInBlockStore();
  });

  test("nextMetaId counts one per meta the board has seen", () => {
    handleNewMetaV1Events([
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        firstCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        secondCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        thirdCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    ]);

    assert.entityCount(ENTITY_TYPE_META_BOARD, 1);
    assert.entityCount(ENTITY_TYPE_META_V1, 3);

    const board = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    assert.bigIntEquals(board.nextMetaId, BigInt.fromI32(3));
  });

  // Cross-board MetaV1 identity is #206's; this asserts only the counters.
  test("Each metaboard counts only its own metas", () => {
    handleNewMetaV1Events([
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        firstCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
      createNewMetaV1Event(
        OTHER_CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        secondCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
      createNewMetaV1Event(
        CONTRACT_ADDRESS,
        sender,
        subject,
        Bytes.fromHexString(metaString),
        thirdCounterTransactionHash,
        transactionBlockNumber,
        transactionTimestamp,
      ),
    ]);

    assert.entityCount(ENTITY_TYPE_META_BOARD, 2);

    const board = MetaBoard.load(CONTRACT_ADDRESS) as MetaBoard;
    const otherBoard = MetaBoard.load(OTHER_CONTRACT_ADDRESS) as MetaBoard;
    assert.bigIntEquals(board.nextMetaId, BigInt.fromI32(2));
    assert.bigIntEquals(otherBoard.nextMetaId, BigInt.fromI32(1));
  });
});
