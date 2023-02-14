#!/bin/bash
# Script for syncing branch protection rules between repos.
# By default copies rules from the `apollo-template` repo to specified branch in the current repo.

if [[ -z $1 || -z $2 ]]; then
    echo "Usage: ./bpsync.sh <GitHub PAT> <target repo> <target branch>"
    exit 1
fi

# Dependency check
DEPS=(jq curl)
for dep in $DEPS; do
    if [ ! $(which $dep) ]; then
        echo "'$dep' is not installed. Exiting."
        exit 1
    fi
done

# personal access token
PAT=$1
# source/target branch for copying protection rules
BRANCH=$2

SRC_OWNER=apollodao
SRC_REPO=apollo-template
SRC_BRANCH=$BRANCH
SRC_PAT=$PAT

TGT_OWNER=apollodao
TGT_REPO=$(basename $(git rev-parse --show-toplevel))
TGT_BRANCH=$BRANCH
TGT_PAT=$PAT

# GET branch protection rules
HTTP_RESP=$(curl \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer ${SRC_PAT}"\
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -w '%{http_code}' \
  -o GET_response.tmp \
  https://api.github.com/repos/$SRC_OWNER/$SRC_REPO/branches/$SRC_BRANCH/protection \
    2>/dev/null
)

if [ $HTTP_RESP != 200 ]; then
    echo "Failed to get branch protection rules!"
    printf "HTTP Response %s\n" $HTTP_RESP
    exit 1
fi

echo "Successfully fetched branch protection rules!"

# Prepare branch protection rules for PUT request
PAYLOAD=$(
    cat GET_response.tmp \
    | jq -c \
        'del(
            .required_signatures,   # Delete "required_signatures"
            ..|                     # Recurse..
                .url?,              # ..and delete "url",
                .contexts_url?,     # "contexts_url" and "contexts"
                .contexts?          # fields in the JSON
        )
        | walk(                     # Recurse again and flatten
                                    # objects with one field
            if type == "object" and length == 1 then
                .[]
            else
                .
            end
        )
        | .restrictions |= null     # Add "restrictions" field'
)

# Try updating branch protection with shiny new JSON payload
HTTP_RESP=$(curl \
  -X PUT \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer ${TGT_PAT}"\
  -H "X-GitHub-Api-Version: 2022-11-28" \
  -w '%{http_code}' \
  -o PUT_response.tmp \
  https://api.github.com/repos/$TGT_OWNER/$TGT_REPO/branches/$TGT_BRANCH/protection \
  -d "${PAYLOAD}" \
    2>/dev/null
)

if [ $HTTP_RESP != 200 ]; then
    echo "Failed to copy branch protection rules!"
    echo "HTTP Response ${HTTP_RESP}"
    echo "ERROR MESSAGE: $(cat PUT_response.tmp | jq '.message')"
    exit 1
fi

echo "Successfully copied branch protection rules!"
# Clean up responses if successful
rm GET_response.tmp PUT_response.tmp
exit 0
