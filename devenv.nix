{ pkgs, lib, ... }:

{
  packages = [
    pkgs.cocogitto
    pkgs.git
    pkgs.openssl
  ] ++ lib.optionals pkgs.hostPlatform.isDarwin [
    pkgs.darwin.apple_sdk.frameworks.Security
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
  };
}
