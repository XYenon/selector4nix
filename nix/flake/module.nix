{
  config,
  flake-parts-lib,
  withSystem,
  ...
}:
{
  flake = {
    darwinModules = {
      default = config.flake.darwinModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../modules/darwin.nix {
        inherit withSystem;
      };
    };

    homeManagerModules = {
      default = config.flake.homeManagerModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../modules/home-manager.nix {
        inherit withSystem;
      };
    };

    nixosModules = {
      default = config.flake.nixosModules.selector4nix;
      selector4nix = flake-parts-lib.importApply ../modules/nixos.nix {
        inherit withSystem;
      };
    };
  };
}
