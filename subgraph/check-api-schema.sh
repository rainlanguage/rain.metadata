#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
snapshot=crates/metaboard/src/schema/metaboard.graphql
generated=metaboard.graphql.generated

# A temp copy: `graph build --network` writes that network's address and
# startBlock back into the manifest it builds, and the source manifest is the
# template that carries neither (rainlanguage/rain.metadata#149).
tree="$(mktemp -d)"
mkdir -p "$tree/subgraph"
cp -R "$root/subgraph/." "$tree/subgraph/"
rm -rf "$tree/subgraph/node_modules" "$tree/subgraph/generated" "$tree/subgraph/build"

while read -r abi; do
  mkdir -p "$(dirname "$tree/subgraph/$abi")"
  cp "$root/subgraph/$abi" "$tree/subgraph/$abi"
done < <(yq -r '.dataSources[].mapping.abis[].file' "$root/subgraph/subgraph.yaml")

yq -r '.dataSources[].name' "$root/subgraph/subgraph.yaml" | jq -R -s '
  {
    anvil: (
      split("\n")
      | map(select(length > 0))
      | map({
          (.): {
            address: "0x0000000000000000000000000000000000000000",
            startBlock: 0
          }
        })
      | add
    )
  }
' > "$tree/subgraph/networks.json"

cd "$tree/subgraph"
npm ci
graph codegen
graph build --network anvil
graph create --node http://localhost:8020/ rain/metaboard
graph deploy \
  --node http://localhost:8020/ \
  --ipfs http://localhost:5001 \
  --version-label ci \
  rain/metaboard

# Nothing has to index: graph-node derives the API schema at deploy time and
# introspection answers it against an empty store.
node ./print-api-schema.js http://localhost:8000/subgraphs/name/rain/metaboard \
  > "$root/$generated"

cd "$root"
if ! diff -u "$snapshot" "$generated"; then
  echo "$snapshot is not the API schema graph-node derives from subgraph/schema.graphql." >&2
  echo "Copy $generated (uploaded by this job as an artifact) over it." >&2
  exit 1
fi
