#!/usr/bin/env sh
# Fetches pinned checkouts of org-mode and Worg for tests/corpus.rs.
# org-mode comes from the emacs-straight mirror of Savannah, which is more
# reliable from CI.
set -eu

root=${1:-target/corpus}

fetch() {
    name=$1
    url=$2
    revision=$3
    destination="$root/$name"
    if [ -d "$destination/.git" ] && [ "$(git -C "$destination" rev-parse HEAD)" = "$revision" ]; then
        return
    fi
    rm -rf "$destination"
    git init -q "$destination"
    git -C "$destination" fetch -q --depth=1 "$url" "$revision"
    git -C "$destination" checkout -q --detach FETCH_HEAD
}

mkdir -p "$root"
fetch org-mode https://github.com/emacs-straight/org-mode 75234089cd3f3229a9becf3ffb334b3df212a57e
fetch worg https://git.sr.ht/~bzg/worg fc5e668072052c75909eb8fba64a6c72c40780a8
printf '%s\n' "$root"
