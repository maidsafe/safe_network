#!/usr/bin/env bash

# Grabs foundation cash note from a server and attempts to deposit it locally via the current installed safe version

# Suffix to append to the version. Passed as an argument to this script.
FOUNDATION_SERVER="$1"

# if doundation server not provided, exit
 if [ -z "$FOUNDATION_SERVER" ]; then
    echo "Please provide the foundation server IP address as an argument to this script"
    exit 1
fi

scp root@$FOUNDATION_SERVER:/home/safe/.local/share/safe/test_faucet/wallet/foundation_cashnote.cash_note $TMPDIR/foundation.cash_note
safe wallet receive $TMPDIR/foundation.cash_note
safe wallet balance
