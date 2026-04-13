#!/usr/bin/env sh
set -eu

if [ "$#" -ne 4 ]; then
  echo "usage: $0 <binary-path> <tag-name> <repository> <output-dir>" >&2
  exit 1
fi

binary_path=$1
tag_name=$2
repository=$3
output_dir=$4
version=${tag_name#v}

archive_name=moove-windows-x86_64.zip
archive_path=$output_dir/$archive_name
checksum_path=$archive_path.sha256
installer_url=https://github.com/$repository/releases/download/$tag_name/$archive_name
package_identifier=Urin.moove

mkdir -p "$output_dir"
rm -f "$archive_path" "$checksum_path"

zip -j "$archive_path" "$binary_path"
installer_sha256=$(sha256sum "$archive_path" | cut -d' ' -f1)
printf "%s  %s\n" "$installer_sha256" "$archive_name" > "$checksum_path"

cat > "$output_dir/Urin.moove.yaml" <<EOF
PackageIdentifier: $package_identifier
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.9.0
EOF

cat > "$output_dir/Urin.moove.installer.yaml" <<EOF
PackageIdentifier: $package_identifier
PackageVersion: $version
Installers:
- Architecture: x64
  InstallerType: zip
  NestedInstallerType: portable
  NestedInstallerFiles:
  - RelativeFilePath: moove.exe
    PortableCommandAlias: moove
  InstallerUrl: $installer_url
  InstallerSha256: $installer_sha256
ManifestType: installer
ManifestVersion: 1.9.0
EOF

cat > "$output_dir/Urin.moove.locale.en-US.yaml" <<EOF
PackageIdentifier: $package_identifier
PackageVersion: $version
PackageLocale: en-US
Publisher: Urin
PublisherUrl: https://github.com/urin
PublisherSupportUrl: https://github.com/urin/moove/issues
Author: Urin
PackageName: moove
PackageUrl: https://github.com/urin/moove
License: MIT OR Apache-2.0
LicenseUrl: https://github.com/urin/moove/blob/main/LICENSE-MIT
ShortDescription: Manipulate file names and locations from your text editor.
Description: Rename and move files and directories by editing a generated list in your preferred text editor.
Moniker: moove
Tags:
- cli
- move
- rename
- rust
ReleaseNotesUrl: https://github.com/urin/moove/releases/tag/$tag_name
ManifestType: defaultLocale
ManifestVersion: 1.9.0
EOF
