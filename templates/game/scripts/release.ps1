# Monta uma versão do mod no seu PC e, com -Publicar, cria o release no GitHub.
# Um mod de jogo existente compila contra os arquivos do jogo instalado, que não podem ir para o GitHub: por isso
# a versão sai daqui, e não do GitHub Actions.
#
#   powershell -File scripts\release.ps1             # compila, confere e empacota em release\ (pa release)
#   powershell -File scripts\release.ps1 -Publicar   # e cria o release vX.Y.Z (pa release --publish; gh logado)
param([switch]$Publicar)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

$version = (Get-Content party-anywhere.json -Raw | ConvertFrom-Json).version
$tag = "v$version"

Write-Host "1/2 Compilando o mod $tag"
# Troque pelo build do seu mod, com tudo o que vai para o jogador na pasta build\ (no caminho em que fica dentro
# da pasta do jogo). Ex.: dotnet build src -c Release; msbuild; cmake --build.
if (-not (Test-Path build)) { throw "Coloque aqui o build do seu mod, com a saída em build\." }

Write-Host "2/2 Conferindo e empacotando"
if ($Publicar) { pa release --from build --publish } else { pa release --from build }
if ($LASTEXITCODE -ne 0) { throw "Veja acima o que a pa disse." }
