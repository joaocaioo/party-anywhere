# Monta uma versão do mod no seu PC e, com -Publicar, cria o release no GitHub.
# Um mod de jogo existente compila contra os arquivos do jogo instalado, que não podem ir para o GitHub: por isso
# a versão sai daqui, e não do GitHub Actions.
#
#   powershell -File scripts\release.ps1             # compila, confere e empacota em release\
#   powershell -File scripts\release.ps1 -Publicar   # e cria o release vX.Y.Z com o pacote (precisa do gh logado)
param([switch]$Publicar)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

$version = (Get-Content party-anywhere.json -Raw | ConvertFrom-Json).version
$tag = "v$version"

Write-Host "1/3 Compilando o mod $tag"
# Troque pelo build do seu mod, com tudo o que vai para o jogador na pasta build\ (no caminho em que fica dentro
# da pasta do jogo). Ex.: dotnet build src -c Release; msbuild; cmake --build.
if (-not (Test-Path build)) { throw "Coloque aqui o build do seu mod, com a saída em build\." }

Write-Host "2/3 Conferindo e empacotando"
pa validate
if ($LASTEXITCODE -ne 0) { throw "A ficha do mod tem problemas (veja acima)." }
pa pack --from build --tag $tag
if ($LASTEXITCODE -ne 0) { throw "O empacotamento falhou." }

if (-not $Publicar) {
    Write-Host "Pronto em release\. Para testar no seu Hub: pa add .   Para publicar: scripts\release.ps1 -Publicar"
    exit 0
}

Write-Host "3/3 Publicando o release $tag"
$files = Get-ChildItem release -File | ForEach-Object { $_.FullName }
gh release create $tag @files --title $tag --notes "Instale pelo Party Anywhere: Biblioteca > Adicionar > Mod co-op por link, com o link deste repositório."
if ($LASTEXITCODE -ne 0) { throw "O gh não criou o release (está logado? a tag já existe?)." }
Write-Host "Publicado: $tag. O repositório precisa ser público para outras pessoas instalarem pelo link."
