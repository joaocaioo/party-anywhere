// PhoneViews.cs: a tela do jogo no celular num jogo Unity, pronta. Vem do mod do MicroWorks (Unity 2019.3, HDRP),
// onde roda com 1 a 10 jogadores. Precisa do PartyAnywhere.cs e de "permitir código unsafe".
//
// Quem ligou "Tela do jogo no celular" sai da TV: a câmera dele desenha numa imagem pequena, no formato da tela
// do celular dele, lida da placa de vídeo sem esperar (leitura assíncrona) e entregue ao Hub. Uso:
//
//   PhoneViews.Hub = hub;                              // o Client conectado
//   gameObject.AddComponent<PhoneViews>();             // uma vez
//   // a cada quadro, para a câmera de cada jogador:
//   if (PhoneViews.Wants(seat)) cam.targetTexture = PhoneViews.TargetFor(seat);   // fora da tela dividida
//   else { PhoneViews.Release(seat); cam.targetTexture = null; cam.rect = suaParteDaTela; }
//
// O HUD do jogador que está no celular precisa desenhar na imagem dele: passe o Canvas para ScreenSpaceCamera com
// essa câmera (e de volta quando ele voltar para a TV). O CanvasScaler mede a janela, não a imagem: compense.
using System.Collections;
using System.Collections.Generic;
using Unity.Collections.LowLevel.Unsafe;
using UnityEngine;
using UnityEngine.Rendering;

namespace PartyAnywhere
{
    public sealed class PhoneViews : MonoBehaviour
    {
        /// <summary>O Client conectado ao Hub (null: ninguém no celular).</summary>
        public static Client Hub;

        // uma leitura leva de 1 a 3 quadros para voltar: várias ao mesmo tempo, uma pedida a cada quadro (o Hub
        // manda até 30 por segundo ao celular, sempre o mais novo)
        const int MaxReading = 3;

        static readonly RenderTexture[] targets = new RenderTexture[Client.MaxSeats];
        // leituras em andamento por imagem: uma imagem trocada (formato novo, volta para a TV) só some quando a
        // última leitura dela volta
        static readonly Dictionary<RenderTexture, int> reading = new Dictionary<RenderTexture, int>();

        /// <summary>O jogador desta vaga quer a parte dele no celular.</summary>
        public static bool Wants(int seat) => Hub != null && Hub.ViewWanted(seat);

        /// <summary>A imagem onde a câmera de quem está no celular desenha (no formato da tela dele).</summary>
        public static RenderTexture TargetFor(int seat)
        {
            Hub.ViewSize(seat, out int w, out int h);
            RenderTexture rt = targets[seat];
            if (rt != null && rt.width == w && rt.height == h) return rt;
            Release(seat); // o celular girou ou mudou de formato
            rt = new RenderTexture(w, h, 24, RenderTextureFormat.ARGB32, RenderTextureReadWrite.sRGB) { name = "PartyAnywhere.Phone.P" + (seat + 1) };
            rt.Create();
            return targets[seat] = rt;
        }

        /// <summary>Voltou para a TV (ou saiu): a imagem dele não é mais desenhada nem entregue.</summary>
        public static void Release(int seat)
        {
            RenderTexture rt = targets[seat];
            if (rt == null) return;
            targets[seat] = null;
            if (!reading.ContainsKey(rt)) Destroy(rt); // senão, a última leitura em andamento termina e solta
        }

        IEnumerator Start()
        {
            var endOfFrame = new WaitForEndOfFrame();
            while (true)
            {
                yield return endOfFrame;
                for (int seat = 0; seat < Client.MaxSeats; seat++)
                {
                    RenderTexture rt = targets[seat];
                    if (rt == null) continue;
                    reading.TryGetValue(rt, out int n);
                    if (n >= MaxReading) continue;
                    reading[rt] = n + 1;
                    int s = seat;
                    AsyncGPUReadback.Request(rt, 0, TextureFormat.BGRA32, r => Deliver(s, rt, r));
                }
            }
        }

        static unsafe void Deliver(int seat, RenderTexture rt, AsyncGPUReadbackRequest r)
        {
            int left = reading[rt] - 1;
            if (left == 0) reading.Remove(rt); else reading[rt] = left;
            if (targets[seat] != rt) // voltou para a TV (ou mudou de formato) enquanto lia
            {
                if (left == 0 && rt != null) Destroy(rt);
                return;
            }
            if (r.hasError || Hub == null) return;
            var data = r.GetData<byte>();
            var p = (byte*)NativeArrayUnsafeUtility.GetUnsafeReadOnlyPtr(data);
            // a imagem vem de baixo para cima; o Hub quer de cima para baixo: começa da última linha e anda para trás
            int row = r.width * 4;
            Hub.SubmitView(seat, p + (long)(r.height - 1) * row, r.width, r.height, -row);
        }
    }
}
