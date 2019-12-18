#!/bin/bash

# Copyright (C) 2019  Braiins Systems s.r.o.
#
# This file is part of Braiins Open-Source Initiative (BOSI).
#
# BOSI is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
#
# Please, keep in mind that we may also license BOSI or any part thereof
# under a proprietary license. For more information on the terms and conditions
# of such proprietary license or if you have any other questions, please
# contact us at opensource@braiins.com.

# this script is used to automate disk image creation
# usage: mkimage.sh [source [dest]]
#   source is directory with files for u-boot
#   dest is file or directory (default name will be used) for resulting image
#
# image contains a fat32 partition and ext4 partition of fixed size
#   fat partition is for uBoot stuff (os image, uEnv.txt and such)
#   ext partition holds upgrade files (TBD)
#
# image creation requires severa additional tools to be installed:
#   sfdisk (by default in debian)
#   mkfs.ext (ditto)
#   mkfs.vfat (from dosfstools)
#   mcopy (mtools)

# perform these steps to build and prepare artifacts to work with
# ./bb.py --platform $target prepare
# ./bb.py --platform $target clean
# ./bb.py --platform $target prepare --update-feeds
# ./bb.py --platform $target build --key keys/test
# ./bb.py --platform $target deploy local_sd

set -e

die() { echo "$@" ; exit 1; }

# image source dir (created by bb)
src=${1:-output/zynq-am1-s9/sd}
# name or dir for created image
dest=${2:-output/zynq-am1-s9}

test -d $src || die "source must be a directory"

# cook up a default name if dest is a directory
if [ -d $dest ]; then
    version=$(./bb.py build-version)
    target="am1-s9"
    # default name for created image (ie. braiins-os_am1-s9_sd_2019-06-05-0-0de55997.img )
    image="braiins-os_${target}_sd_${version}.img"
    dest=$dest/$image
fi
echo packing $src into $dest
tmpdir=$(mktemp -d)

# os partition size in MB (fat one)
image_ospart_size=14
# upgrade partition size in MB (fat one)
image_upgpart_size=32

# first partition (fat)
dd if=/dev/zero of=$tmpdir/part1.img bs=1M count=$image_ospart_size
mkfs.vfat $tmpdir/part1.img -n braiins-os
mcopy -i $tmpdir/part1.img $src/* ::/

# recovery mode
# TODO: this ought to be moved to bb script or have base per-target uEnv.txt commited in repo
mtdparts="mtdparts=pl35x-nand:512k(boot),2560k(uboot),2m(fpga1),2m(fpga2),512k(uboot_env),512k(miner_cfg),22m(recovery),95m(firmware1),95m(firmware2)"
recovery_mtdparts="${mtdparts},144m@0x2000000(antminer_rootfs)"
cp $src/uEnv.txt $tmpdir/
echo "recovery_mtdparts='$recovery_mtdparts'" >> $tmpdir/uEnv.txt
mcopy -o -i $tmpdir/part1.img $tmpdir/uEnv.txt ::/

# second partition (ext)
dd if=/dev/zero of=$tmpdir/part2.img bs=1M count=$image_upgpart_size
mkdir $tmpdir/part2
date +'%Y-%m-%d' > $tmpdir/part2/created    # TODO upgrade stuff belongs here
mkfs.ext4 $tmpdir/part2.img -d $tmpdir/part2 -L braiins-upgrade

# assemble whole image
# note that partitions are aligned to blocksize of following dd commands
dd if=/dev/zero of=$tmpdir/part.img bs=1M count=$((1 + $image_ospart_size + $image_upgpart_size))
sfdisk $tmpdir/part.img <<-EOF
    start=1M, size=${image_ospart_size}M, type=c
    size=${image_upgpart_size}M, type=83
EOF
dd if=$tmpdir/part1.img of=$tmpdir/part.img bs=1M seek=1 count=${image_ospart_size} conv=notrunc
dd if=$tmpdir/part2.img of=$tmpdir/part.img bs=1M seek=$((1 + $image_ospart_size)) count=$(($image_upgpart_size)) conv=notrunc

mv $tmpdir/part.img $dest

rm -r $tmpdir
