# Mesure de vitesse stable : priorite haute et coeur fige, la machine etant
# rarement au repos. Rend la meilleure des N passes, celle la moins polluee.
#
# Le materiel de travail vient de l'environnement, jamais du script :
#   $env:SONIX_DEVICE_KEY = "0x........"
#   $env:SONIX_DUMPS      = "<dossier des .bin>"
#   $env:SONIX_ETAT       = "<chemin d'un .tamastate>"
param(
  [string]$Exe = "target\release\examples\vitesse_probe.exe",
  [int]$Passes = 3,
  [int]$Secondes = 5,
  [string]$Dump = "Tamagotchi_Paradise_Water_MX25L12835F.bin"
)
if (-not $env:SONIX_DEVICE_KEY) { throw "SONIX_DEVICE_KEY non definie" }
if (-not $env:SONIX_DUMPS)      { throw "SONIX_DUMPS non definie" }
if (-not $env:SONIX_ETAT)       { throw "SONIX_ETAT non definie" }
$chemin = Join-Path $env:SONIX_DUMPS $Dump
$best = @{}
for ($i = 0; $i -lt $Passes; $i++) {
  $p = Start-Process -FilePath $Exe `
       -ArgumentList $chemin, $env:SONIX_DEVICE_KEY, $env:SONIX_ETAT, $Secondes `
       -NoNewWindow -PassThru -RedirectStandardOutput "$env:TEMP\vp$i.txt"
  $p.PriorityClass = "High"
  $p.ProcessorAffinity = 16
  $p.WaitForExit()
  foreach ($l in Get-Content "$env:TEMP\vp$i.txt") {
    if ($l -match '^\s+(\S+)\s+:.*soit ([0-9.]+) fois') {
      $k = $matches[1]; $v = [double]$matches[2]
      if (-not $best.ContainsKey($k) -or $v -gt $best[$k]) { $best[$k] = $v }
    }
  }
}
foreach ($k in $best.Keys | Sort-Object) { "{0,-12} {1:N2}" -f $k, $best[$k] }
