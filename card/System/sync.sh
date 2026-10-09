#!/bin/sh
# Keeps Saves/ in step with another device (the Thor) over your home Wi-Fi, using
# Syncthing. Slot runs this only while you are on the shelf and stops it before a
# game starts, so it never shares the CPU with a running game.
#
#   sync.sh start     join Wi-Fi from System/wifi.conf and start Syncthing
#   sync.sh stop      stop Syncthing and let go of the Wi-Fi
#   sync.sh status    print one word: off starting running no-binary no-wifi no-ip error
#   sync.sh pair [S]  for S seconds (default 120) accept the first device that asks
#   sync.sh id        print this device's Syncthing ID
set -u

SD="${SLOT_ROOT:-/mnt/sdcard}"
SYS="$SD/System"
RUN="${AGS_RUN:-/run}"
DATA="${AGS_DATA:-/data}"
ST="${SYNC_BIN:-$SYS/syncthing}"
NETCTL="${AGS_NET:-/usr/sbin/ags-net}"
IPBIN="${AGS_IP:-ip}"
WPA_CLI="${AGS_WPA_CLI:-wpa_cli}"
FOLDER_ID="slot-saves"
WAIT_API="${SYNC_WAIT_API:-60}"
WAIT_IP="${SYNC_WAIT_IP:-60}"

STATE="$RUN/slot-sync.state"
PIDF="$RUN/slot-sync.pid"
WANT="$RUN/slot-sync.want"
IDF="$RUN/slot-sync.id"
LOG="$SD/sync.log"
PEER_FILE="$SD/Config/sync_peer.txt"
ID_FILE="$SD/Config/sync_id.txt"

DBG="$SD/sync-debug.log"
dbg() { echo "$(date '+%H:%M:%S') $*" >> "$DBG" 2>/dev/null; }
say() { echo "$1" > "$STATE" 2>/dev/null; dbg "state: $1"; }

# Where Syncthing keeps its identity and database. /data survives card swaps, so
# the pairing does too. A card with no writable /data keeps it on the card.
pick_home() {
	if mkdir -p "$DATA/syncthing" 2>/dev/null && [ -w "$DATA/syncthing" ]; then
		H="$DATA/syncthing"
	else
		H="$SD/Config/syncthing"
		mkdir -p "$H"
	fi
	export STHOMEDIR="$H"
	export STNOUPGRADE=1
	export STNODEFAULTFOLDER=1
	export STNORESTART=1
}

st_cli() { "$ST" cli "$@"; }

running() {
	pid="$(cat "$PIDF" 2>/dev/null)"
	[ -n "$pid" ] && [ -d "/proc/$pid" ]
}

has_ip() {
	$IPBIN -4 addr show wlan0 2>/dev/null | grep -q 'inet '
}

device_id() {
	out="$("$ST" --device-id 2>/dev/null | ids_in | head -n 1)"
	[ -n "$out" ] || out="$("$ST" device-id 2>/dev/null | ids_in | head -n 1)"
	echo "$out"
}

ids_in() {
	grep -oE '[A-Z2-7]{7}(-[A-Z2-7]{7}){7}'
}

share_with() {
	peer="$1"
	[ -n "$peer" ] || return 0
	st_cli config devices add --device-id "$peer" --name "Thor" >/dev/null 2>&1 || true
	st_cli config folders "$FOLDER_ID" devices add --device-id "$peer" >/dev/null 2>&1 || true
}

first_time_setup() {
	[ -f "$H/.slot-configured" ] && return 0
	st_cli config folders add --id "$FOLDER_ID" --label "Slot Saves" --path "$SD/Saves" \
		>/dev/null 2>&1 || return 1
	: > "$H/.slot-configured"
}

# Anything the card file names is shared with, every start. Safe to repeat.
apply_peer_file() {
	[ -f "$PEER_FILE" ] || return 0
	for p in $(ids_in < "$PEER_FILE"); do
		share_with "$p"
	done
}

start() {
	dbg "start requested"
	if running; then
		return 0
	fi
	[ -x "$ST" ] || [ -f "$ST" ] || { say no-binary; return 1; }
	[ -f "$SYS/wifi.conf" ] || { say no-wifi; return 1; }
	say starting
	: > "$WANT"
	pick_home

	if ! has_ip; then
		dbg "joining wifi with $NETCTL"
		"$NETCTL" wifi >> "$DBG" 2>&1
		dbg "ags-net wifi returned $?; ip: $($IPBIN -4 addr show wlan0 2>&1 | grep inet)"
		n=0
		while ! has_ip && [ "$n" -lt "$WAIT_IP" ]; do
			sleep 1
			n=$((n + 1))
		done
		has_ip || { say no-ip; return 1; }
	fi
	[ -f "$WANT" ] || return 1

	dbg "have ip, home $H"
	if [ ! -f "$H/config.xml" ]; then
		dbg "generating keys"
		"$ST" generate --home "$H" --no-default-folder >/dev/null 2>&1 \
			|| "$ST" generate --home "$H" >/dev/null 2>&1
		# Home network only: no global discovery, relays or router port mapping.
		if [ -f "$H/config.xml" ]; then
			sed -i \
				-e 's|<globalAnnounceEnabled>true<|<globalAnnounceEnabled>false<|' \
				-e 's|<relaysEnabled>true<|<relaysEnabled>false<|' \
				-e 's|<natEnabled>true<|<natEnabled>false<|' \
				"$H/config.xml"
		fi
	fi

	[ -f "$LOG" ] && mv -f "$LOG" "$LOG.1"
	dbg "starting syncthing"
	"$ST" serve --no-browser --no-restart --gui-address=127.0.0.1:8384 \
		> "$LOG" 2>&1 &
	echo $! > "$PIDF"

	n=0
	until st_cli show system >/dev/null 2>&1; do
		running || { say error; return 1; }
		n=$((n + 1))
		[ "$n" -lt "$WAIT_API" ] || { say error; return 1; }
		sleep 1
	done

	dbg "syncthing api is up"
	first_time_setup || { say error; return 1; }
	apply_peer_file

	id="$(device_id)"
	if [ -n "$id" ]; then
		echo "$id" > "$IDF"
		echo "$id" > "$ID_FILE" 2>/dev/null
	fi
	say running
}

stop() {
	rm -f "$WANT"
	if running; then
		pid="$(cat "$PIDF")"
		kill "$pid" 2>/dev/null
		n=0
		while [ -d "/proc/$pid" ] && [ "$n" -lt 10 ]; do
			sleep 0.5 2>/dev/null || sleep 1
			n=$((n + 1))
		done
		[ -d "/proc/$pid" ] && kill -9 "$pid" 2>/dev/null
	fi
	rm -f "$PIDF"
	# Let go of the Wi-Fi unless the card asked for the network at boot.
	if [ ! -f "$SD/System/network.on" ] && [ ! -f "$SD/System/ssh.on" ]; then
		$WPA_CLI -i wlan0 terminate >/dev/null 2>&1
		$IPBIN addr flush dev wlan0 >/dev/null 2>&1
	fi
	sync
	say off
}

status() {
	if running; then
		cat "$STATE" 2>/dev/null || echo running
		return 0
	fi
	case "$(cat "$STATE" 2>/dev/null)" in
	no-binary | no-wifi | no-ip | error) cat "$STATE" ;;
	*) echo off ;;
	esac
}

pair() {
	secs="${1:-120}"
	running || { echo "not running" >&2; return 1; }
	pick_home
	left="$secs"
	while [ "$left" -gt 0 ]; do
		for p in $(st_cli show pending devices 2>/dev/null | ids_in); do
			share_with "$p"
			echo "paired $p"
			return 0
		done
		sleep 2
		left=$((left - 2))
	done
	return 1
}

case "${1:-}" in
start) start ;;
stop) stop ;;
status) status ;;
pair) shift; pair "$@" ;;
id) cat "$IDF" 2>/dev/null || cat "$ID_FILE" 2>/dev/null ;;
*)
	echo "usage: sync.sh [start|stop|status|pair [seconds]|id]" >&2
	exit 2
	;;
esac
