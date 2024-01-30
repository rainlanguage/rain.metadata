{
  description = "Flake for development workflows.";

  inputs = {
    rainix.url = "github:rainprotocol/rainix";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {self, flake-utils, rainix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = rainix.pkgs.${system};
        rust-toolchain = rainix.rust-toolchain.${system};
      in {
        packages = {
          mkBin = (pkgs.makeRustPlatform{
            rustc = rust-toolchain;
            cargo = rust-toolchain;
          }).buildRustPackage {
            src = ./.;
            doCheck = false;
            name = "rain-metadata";
            cargoLock.lockFile = ./Cargo.lock;
            # allows for git deps to be resolved without the need to specify their outputHash
            cargoLock.allowBuiltinFetchGit = true;
            buildPhase = ''
              cargo build --release --bin rain-metadata --all-features
            '';
            installPhase = ''
              mkdir -p $out/bin
              cp target/release/rain-metadata $out/bin/
            '';
            buildInputs = with pkgs; [
              openssl
            ];
            nativeBuildInputs = with pkgs; [
              pkg-config
            ] ++ lib.optionals stdenv.isDarwin [
              darwin.apple_sdk.frameworks.SystemConfiguration
            ];
          };
        } // rainix.packages.${system};
        devShells = rainix.devShells.${system};
      }
    );

}
