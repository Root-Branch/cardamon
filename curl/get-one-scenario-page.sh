#!/bin/sh

curl 'http://127.0.0.1:7001/api/scenarios' > scenarios.json
jq . scenarios.json > scenarios-formatted.json
rm scenarios.json
mv scenarios-formatted.json scenarios.json

echo "List of Scenarios:"
jq -r '.scenarios[] | "ID: \(.name)"' scenarios.json | nl -v 1

echo "Please enter the number of the scenario you want to select:"
read index
echo "Enter page number:"
read page_number
scenario_id=$(jq -r ".scenarios[$((index - 1))].name" scenarios.json)

curl "http://127.0.0.1:7001/api/scenarios/${scenario_id}?page=$page_number" > specific_scenario.json
jq . specific_scenario.json > specific_scenario_formatted.json
rm specific_scenario.json
mv specific_scenario_formatted.json specific_scenario.json

nvim specific_scenario.json

