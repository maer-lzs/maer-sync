#!/bin/bash
# Mär — an audio player for self-hosted open ecosystems
# Copyright (C) 2026 Laurenz Schröder
# SPDX-License-Identifier: GPL-3.0-or-later
# Additional terms apply — see LICENSE-ADDITIONAL-TERMS
#
# server-tor.sh: the gate of the server, installed in Forgejo as
# hooks/pre-receive.d/20-regeln of maer-sync.git with the patterns beside it
# as 20-regeln.muster. It reads every push, whatever clone it comes from:
# access values in added lines, the author, attribution trailers, no deletion
# of dev and no rewrite of it, and the note of a green run at the head of
# every branch but main. Not built yet: it lets every push through.
cat > /dev/null
exit 0
