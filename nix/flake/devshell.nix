{
  inputs,
  self,
  ...
}:
{
  perSystem =
    { config, pkgs, ... }:
    {
      devShells.default = pkgs.mkShellNoCC {
        packages = [
          config.packages.rust-toolchain
          pkgs.nix-serve-ng
          pkgs.nixfmt
          pkgs.nixfmt-tree
        ];
      };
    };
}
