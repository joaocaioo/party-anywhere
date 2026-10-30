@echo off
rem Mede a latencia do celular como controle (fase 1). Clique duas vezes neste arquivo.
chcp 65001 >nul
cd /d "%~dp0"
title Party Anywhere - medir latencia
echo.
echo  Abrindo o medidor. Na primeira vez ele compila (1 ou 2 minutos)...
echo.
cargo run --release -q -p pa-latency-spike -- --duration 120
echo.
echo  Pronto. Tire um print desta janela e mande para o Claude.
echo.
pause
