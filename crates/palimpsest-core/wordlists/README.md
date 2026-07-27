# Wordlists

One frequency word list per supported language, used by `scoring.rs` for both
the dictionary lookup and the bigram fallback. Format: lowercase, one word per
line, ordered by descending corpus frequency.

`build.rs` compiles these into FSTs at build time. Edit the text files here; the
generated `.fst` files live in `OUT_DIR` and are never committed.

## Source and licence

Derived from [FrequencyWords](https://github.com/hermitdave/FrequencyWords)
(Hermit Dave, MIT licence), `content/2018/{en,de,ru}/{en,de,ru}_50k.txt`, built
from OpenSubtitles corpora.

Palimpsest itself is Apache-2.0; the MIT licence of this data requires that the
attribution above travels with it.

## Regenerating

Filters applied to the upstream `<word> <count>` lists: count at least 50, at
least three characters, and only letters of that language's own alphabet. The
alphabet filter is what keeps Latin loanwords out of the Russian list, which
matters because cross-alphabet separation is what makes the RU↔EN and RU↔DE
comparisons reliable.

```powershell
$alpha = @{ en='^[a-z]+$'; de='^[a-zäöüß]+$'; ru='^[а-яё]+$' }
foreach ($l in 'en','de','ru') {
    $src = "$env:TEMP\${l}_50k.txt"
    Invoke-WebRequest -UseBasicParsing `
        "https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/$l/${l}_50k.txt" `
        -OutFile $src
    $out = [System.Collections.Generic.List[string]]::new()
    foreach ($line in [System.IO.File]::ReadLines($src)) {
        $p = $line.Split(' ')
        if ($p.Count -ne 2) { continue }
        $w = $p[0].ToLowerInvariant()
        if ([int64]$p[1] -lt 50 -or $w.Length -lt 3 -or $w -notmatch $alpha[$l]) { continue }
        $out.Add($w)
    }
    [System.IO.File]::WriteAllLines("$PSScriptRoot\$l.txt", $out,
        (New-Object System.Text.UTF8Encoding $false))
}
```

Current sizes: en 46,278 words, de 49,028, ru 47,951 — about 660 KB of FST once
compiled. After changing a list, re-run `cargo test --workspace` and re-check
the correction margins as described under "Scoring Data" in
`AGENT_INSTRUCTIONS.md`.
