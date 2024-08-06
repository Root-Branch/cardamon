#!/bin/sh

echo "Enter the page number:"
read page_number

curl "http://127.0.0.1:7001/api/scenarios?page=$page_number" > scenarios-page.json
jq . scenarios-page.json > scenarios-formatted.json
rm scenarios-page.json
mv scenarios-formatted.json scenarios-page.json
nvim scenarios-page.json
