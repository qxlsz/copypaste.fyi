{
  description = "copypaste.v1 — ephemeral paste layer for humans and agents";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = nixpkgs.lib.genAttrs systems;
    in {
      packages = forAll (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in {
          default = pkgs.stdenvNoCC.mkDerivation {
            pname = "copypaste";
            version = "1.0.0";
            src = ./.;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            installPhase = ''
              mkdir -p $out/bin $out/share/man/man1 $out/share/doc/copypaste
              cp cli/copypaste.mjs $out/bin/copypaste
              chmod 0755 $out/bin/copypaste
              wrapProgram $out/bin/copypaste --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.nodejs_22 ]}
              cp packaging/man/copypaste.1 $out/share/man/man1/
              cp docs/AGENTS.md docs/SECURITY.md ACCEPTABLE_USE.md $out/share/doc/copypaste/
            '';
            meta = {
              description = "Ephemeral paste layer for humans and AI agents";
              license = pkgs.lib.licenses.asl20;
              platforms = pkgs.lib.platforms.unix;
            };
          };
        });
    };
}
