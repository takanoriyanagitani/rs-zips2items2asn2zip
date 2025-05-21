#!/bin/sh

input0="./sample.d/in.d/z0.zip"
input1="./sample.d/in.d/z1.zip"

ENV_OUT_ZIPNAME=/guest-o.d/out.zip

geninput(){
	echo generating input zips...

	mkdir -p sample.d/in.d

	jq -c -n '{name: "fuji",  height: 3.776}' > sample.d/in.d/z0j0.json
	jq -c -n '{name: "takao", height: 0.599}' > sample.d/in.d/z0j1.json

	jq -c -n '{name: "FUJI",  height: 3.776}' > sample.d/in.d/z1j0.json
	jq -c -n '{name: "TAKAO", height: 0.599}' > sample.d/in.d/z1j1.json

	ls sample.d/in.d/z0*.json |
		zip \
			-0 \
			-@ \
			-T \
			-v \
			-o \
			"${input0}"

	ls sample.d/in.d/z1*.json |
		zip \
			-0 \
			-@ \
			-T \
			-v \
			-o \
			"${input1}"
}

test -f "${input0}" || geninput
test -f "${input1}" || geninput

echo converting zip files to a zip file...
ls \
	"${input0}" \
	"${input1}" |
	cut -d/ -f4- |
	sed \
		-n \
		-e 's,^,/guest-i.d/,' \
		-e p |
	wazero \
		run \
		-env ENV_OUT_ZIPNAME="${ENV_OUT_ZIPNAME}" \
		-mount "${PWD}/sample.d/in.d:/guest-i.d:ro" \
		-mount "${PWD}/sample.d/out.d:/guest-o.d" \
		./basic.wasm

echo
echo printing the created zip file...
unzip -lv sample.d/out.d/out.zip

echo
echo extracting the original info...
unzip -p sample.d/out.d/out.zip /guest-i.d/z0.zip |
	xxd -ps |
	tr -d '\n' |
	python3 \
		-m asn1tools \
		convert \
		-i der \
		-o jer \
		zipitem.asn \
		ZipItems \
		- |
	jq -c '.[]' |
	jq --raw-output -c .data |
	while read line; do
		echo "${line}" | xxd -r -ps | jq -c
	done
