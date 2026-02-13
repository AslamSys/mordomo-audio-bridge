# 🌉 Audio Bridge

**Container:** `audio-bridge`  
**Tecnologia:** Rust (tokio + async-nats + cpal)  
**Ecossistema:** Mordomo  
**Posição no Fluxo:** Reprodução de áudio TTS

**Por que Rust?** Latência crítica (<5ms), zero-copy streaming, sem GC pauses

---

## 📋 Propósito

Recebe chunks de áudio do TTS Engine via NATS e reproduz no alto-falante com latência mínima.

---

## 🎯 Responsabilidades

### Primárias
- ✅ Receber áudio TTS e enviar de volta para Aslam App
- ✅ Gerenciar estado visual (idle, listening, processing, speaking)
- ✅ Publicar chunks de áudio no NATS para wake-word-detector (apenas se necessário loopback)

### Secundárias
- ✅ Suportar múltiplos clients simultâneos (vários tablets/devices)
- ✅ Monitorar qualidade de áudio (ruído, clipping)
- ✅ Auto-ajuste de ganho (AGC)

---

## 🔧 Tecnologias

**Stack:** Rust (Nativo)

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
async-nats = "0.33"
cpal = "0.15"  # Cross-platform audio library (ALSA/WASAPI)
rubato = "0.14" # Resampling de alta qualidade
opus = "0.3"   # Codec para WebRTC
warp = "0.3"   # WebSocket server leve
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
log = "0.4"
env_logger = "0.10"
```

---

## 📊 Especificações

```yaml
Performance:
  CPU: < 1% (Zero-copy streaming)
  RAM: ~ 15 MB
  Latency: < 5ms (Internal processing)
  
Audio:
  Input: WebM/Opus (browser) → PCM 16kHz mono
  Output: PCM 16kHz → WebM/Opus (browser)
  Sample Rate: 16000 Hz
  Bit Depth: 16-bit
  Channels: Mono
  
VAD:
  Algorithm: WebRTC VAD (Rust binding)
  Sensitivity: Medium (ajustável)
  Frame Size: 30ms
```

---

## 🎵 Feedback Sonoro (Earcons)

O `audio-bridge` é responsável por injetar sons de feedback (Earcons) no stream de saída para melhorar a UX sem necessidade de fala.

**Mapeamento de Eventos NATS → Sons:**

| Evento NATS | Arquivo | Descrição |
| :--- | :--- | :--- |
| `wake_word.detected` | `wake.wav` | *Plim!* (Atenção) |
| `llm.processing` | `thinking.wav` | *Tudum...* (Pensando) |
| `action.completed` | `success.wav` | *Bip!* (Sucesso) |
| `system.error` | `error.wav` | *Bop.* (Erro) |

> 📁 **Localização:** Os arquivos `.wav` devem estar em `./assets/sounds/`.

---

## 🔌 Interfaces

### WebSocket (Aslam App ↔ Audio Bridge)

**Client → Server (Áudio do microfone)**

```typescript
// Conexão
ws://audio-bridge:3100/audio?device_id=tablet_sala

// Mensagem de áudio (binário)
{
  type: "audio_chunk",
  data: Uint8Array,  // PCM 16kHz ou WebM
  timestamp: number,
  device_id: "tablet_sala"
}

// Heartbeat (keep-alive)
{
  type: "ping",
  device_id: "tablet_sala"
}
```

**Server → Client (Feedback visual + TTS)**

```typescript
// Estado do sistema
{
  type: "state_changed",
  state: "idle" | "listening" | "processing" | "speaking",
  timestamp: number
}

// Wake word detectada
{
  type: "wake_word_detected",
  confidence: 0.95,
  timestamp: number
}

// Transcrição em tempo real
{
  type: "transcription_partial",
  text: "Qual a tempera...",
  speaker_id: "user_1"
}

// Áudio TTS (binário)
{
  type: "audio_response",
  data: Uint8Array,  // WebM/Opus para reprodução
  text: "A temperatura é 23°C",
  duration_ms: 2500
}

// Erro
{
  type: "error",
  code: "AUDIO_QUALITY_LOW",
  message: "Ruído alto detectado"
}
```

---

### NATS Publications (Envia para pipeline)

```yaml
# Chunks de áudio processados
mordomo.audio.chunk:
  payload:
    data: Buffer  # PCM 16kHz mono
    device_id: tablet_sala
    timestamp: 1732723200.123
    vad_active: true

# Estado de device conectado
mordomo.audio.device_connected:
  payload:
    device_id: tablet_sala
    device_type: browser  # browser | usb_mic
    ip_address: 192.168.1.100
    timestamp: 1732723200.123

# Qualidade de áudio
mordomo.audio.quality_alert:
  payload:
    device_id: tablet_sala
    issue: high_noise | clipping | low_volume
    severity: warning | critical
```

---

### NATS Subscriptions (Recebe do pipeline)

```yaml
# Wake word detectada
mordomo.wake_word.detected:
  payload:
    device_id: tablet_sala
    confidence: 0.95
    timestamp: 1732723200.123
  action: Envia estado "listening" para Aslam App

# Transcrição parcial (streaming)
mordomo.speech.transcription_partial:
  payload:
    text: "Qual a temperatura"
    device_id: tablet_sala
    speaker_id: user_1
  action: Envia para Aslam App mostrar texto

# Transcrição final
mordomo.speech.transcribed:
  payload:
    text: "Qual a temperatura?"
    confidence: 0.95
    device_id: tablet_sala
    speaker_id: user_1
  action: Muda estado para "processing"

# Resposta do Brain
mordomo.brain.response_generated:
  payload:
    text: "A temperatura é 23°C"
    device_id: tablet_sala
  action: Solicita TTS

# Áudio TTS pronto
mordomo.tts.audio_generated:
  payload:
    audio: Buffer  # PCM 16kHz
    text: "A temperatura é 23°C"
    duration_ms: 2500
    device_id: tablet_sala
  action: Converte para WebM e envia via WS
```

---

## ⚙️ Configuração

```yaml
server:
  port: 3100
  host: "0.0.0.0"
  
websocket:
  max_payload: 10485760  # 10MB
  ping_interval: 30000   # 30s
  ping_timeout: 5000     # 5s
  max_clients: 10
  
audio:
  # Input do browser
  input_format: "webm"  # ou "pcm" se já vem processado
  input_codec: "opus"
  
  # Output para processamento
  processing_format: "pcm"
  sample_rate: 16000
  channels: 1
  bit_depth: 16
  
  # VAD
  vad:
    enabled: true
    mode: 2  # 0=quality, 1=low_bitrate, 2=aggressive, 3=very_aggressive
    frame_duration_ms: 30
    
  # AGC (Auto Gain Control)
  agc:
    enabled: true
    target_level: -3  # dB
    
  # Noise Suppression
  noise_suppression:
    enabled: true
    level: moderate  # low | moderate | high | very_high
    
nats:
  url: "nats://nats:4222"
  token_file: "/run/secrets/nats_token"
  max_reconnect: 10
  
fallback:
  # Se WebSocket cair, usar mic USB
  usb_mic:
    enabled: true
    device: "hw:1,0"  # ALSA device
```

---

## 🔒 Segurança (Secrets)

```yaml
# docker-compose.yml
services:
  audio-bridge:
    secrets:
      - nats_token

secrets:
  nats_token:
    file: ./secrets/nats_token.txt
```

---

## 🎨 Integração com Aslam App

### Código JavaScript (Aslam App)

```typescript
// src/services/AudioBridge.ts
export class AudioBridge {
  private ws: WebSocket;
  private mediaStream: MediaStream | null = null;
  private audioContext: AudioContext;
  private processor: ScriptProcessorNode;
  
  constructor() {
    this.audioContext = new AudioContext({ sampleRate: 16000 });
  }
  
  async connect(deviceId: string) {
    // Conectar WebSocket
    this.ws = new WebSocket(`ws://audio-bridge:3100/audio?device_id=${deviceId}`);
    
    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);
      
      switch(msg.type) {
        case 'state_changed':
          this.updateUIState(msg.state);  // idle, listening, processing, speaking
          break;
          
        case 'wake_word_detected':
          this.animateWakeWord();  // Pisca círculo
          break;
          
        case 'transcription_partial':
          this.showPartialText(msg.text);  // "Qual a tempe..."
          break;
          
        case 'audio_response':
          this.playTTS(msg.data);  // Reproduz resposta
          break;
      }
    };
    
    // Pedir permissão de microfone
    this.mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        sampleRate: 16000,
        channelCount: 1,
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true
      }
    });
    
    // Processar áudio e enviar
    const source = this.audioContext.createMediaStreamSource(this.mediaStream);
    this.processor = this.audioContext.createScriptProcessor(4096, 1, 1);
    
    this.processor.onaudioprocess = (e) => {
      const inputData = e.inputBuffer.getChannelData(0);
      
      // Converter Float32 para Int16 PCM
      const pcmData = new Int16Array(inputData.length);
      for (let i = 0; i < inputData.length; i++) {
        const s = Math.max(-1, Math.min(1, inputData[i]));
        pcmData[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
      }
      
      // Enviar para audio-bridge
      this.ws.send(JSON.stringify({
        type: 'audio_chunk',
        data: Array.from(pcmData),
        timestamp: Date.now(),
        device_id: deviceId
      }));
    };
    
    source.connect(this.processor);
    this.processor.connect(this.audioContext.destination);
  }
  
  private updateUIState(state: string) {
    // idle: Círculo cinza estático
    // listening: Círculo verde pulsando
    // processing: Círculo azul girando
    // speaking: Círculo roxo com onda sonora
    
    document.body.className = `state-${state}`;
  }
  
  private animateWakeWord() {
    // Animação: círculo pisca 3x rapidamente
    const circle = document.querySelector('.voice-circle');
    circle?.classList.add('wake-detected');
    setTimeout(() => circle?.classList.remove('wake-detected'), 1000);
  }
  
  private async playTTS(audioData: number[]) {
    const buffer = this.audioContext.createBuffer(1, audioData.length, 16000);
    const channelData = buffer.getChannelData(0);
    
    for (let i = 0; i < audioData.length; i++) {
      channelData[i] = audioData[i] / 32768;  // Int16 → Float32
    }
    
    const source = this.audioContext.createBufferSource();
    source.buffer = buffer;
    source.connect(this.audioContext.destination);
    source.start();
  }
  
  disconnect() {
    this.ws?.close();
    this.mediaStream?.getTracks().forEach(track => track.stop());
    this.processor?.disconnect();
  }
}
```

---

## 🎨 Estados Visuais (Aslam App)

```css
/* Estados do assistente */
.state-idle .voice-circle {
  background: radial-gradient(circle, #4a5568, #2d3748);
  animation: none;
}

.state-listening .voice-circle {
  background: radial-gradient(circle, #48bb78, #38a169);
  animation: pulse 1.5s ease-in-out infinite;
}

.state-processing .voice-circle {
  background: radial-gradient(circle, #4299e1, #3182ce);
  animation: spin 2s linear infinite;
}

.state-speaking .voice-circle {
  background: radial-gradient(circle, #9f7aea, #805ad5);
  animation: wave 0.8s ease-in-out infinite;
}

.wake-detected {
  animation: flash 0.3s ease-in-out 3;
}

@keyframes pulse {
  0%, 100% { transform: scale(1); opacity: 1; }
  50% { transform: scale(1.1); opacity: 0.8; }
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@keyframes wave {
  0%, 100% { transform: scaleY(1); }
  50% { transform: scaleY(1.2); }
}

@keyframes flash {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}
```

---

## 🔄 Fluxo Completo (Tablet → Resposta)

```
1. Usuário abre Aslam App no tablet
   └─> Pede permissão de microfone (popup browser)
   
2. Aslam App conecta ao audio-bridge
   └─> WebSocket: ws://audio-bridge:3100/audio?device_id=tablet_sala
   
3. Microfone captura áudio continuamente
   └─> JavaScript: navigator.mediaDevices.getUserMedia()
   
4. Aslam App processa e envia chunks
   └─> WebSocket → audio-bridge: PCM 16kHz chunks
   
5. audio-bridge aplica VAD
   └─> Se silêncio: descarta
   └─> Se voz ativa: publica mordomo.audio.chunk
   
6. wake-word-detector recebe áudio
   └─> Detecta "ASLAM"
   └─> Publica: mordomo.wake_word.detected
   
7. audio-bridge recebe evento e notifica Aslam App
   └─> WebSocket → Aslam App: {type: "wake_word_detected"}
   └─> Aslam App: Anima círculo (pisca verde 3x)
   
8. Pipeline continua: STT → Brain → TTS
   
9. tts-engine gera áudio PCM
   └─> Publica: mordomo.tts.audio_generated
   
10. audio-bridge recebe áudio TTS
    └─> Converte PCM → WebM/Opus
    └─> WebSocket → Aslam App: {type: "audio_response", data: [...]}
    
11. Aslam App reproduz resposta
    └─> AudioContext.createBufferSource().start()
    └─> Anima círculo (roxo com onda)
    
12. Fim: Volta ao estado idle (cinza)
```

---

## 🐳 Docker

```yaml
services:
  audio-bridge:
    build: ./audio-bridge
    container_name: mordomo-audio-bridge
    ports:
      - "3100:3100"
    environment:
      - NODE_ENV=production
      - NATS_URL=nats://nats:4222
    secrets:
      - nats_token
    depends_on:
      - nats
    networks:
      - mordomo-net
    restart: unless-stopped
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 150M
        reservations:
          cpus: '0.1'
          memory: 80M
    healthcheck:
      test: ["CMD", "node", "healthcheck.js"]
      interval: 30s
      timeout: 5s
      retries: 3
```

---

## 📊 Métricas (Prometheus)

```typescript
import client from 'prom-client';

const audioChunksSent = new client.Counter({
  name: 'audio_chunks_sent_total',
  help: 'Total audio chunks sent to pipeline',
  labelNames: ['device_id']
});

const vadActiveRatio = new client.Gauge({
  name: 'vad_active_ratio',
  help: 'Ratio of voice activity detected',
  labelNames: ['device_id']
});

const wsConnections = new client.Gauge({
  name: 'websocket_connections_active',
  help: 'Active WebSocket connections (devices)',
  labelNames: ['device_type']  // browser, usb_mic
});

const audioLatency = new client.Histogram({
  name: 'audio_bridge_latency_seconds',
  help: 'Latency from receiving audio to publishing on NATS',
  buckets: [0.01, 0.025, 0.05, 0.1, 0.5]
});
```

---

## 🔧 Troubleshooting

### Microfone não captura

```bash
# Verificar permissão no browser
Chrome: chrome://settings/content/microphone
Firefox: about:preferences#privacy

# Testar WebSocket
wscat -c ws://audio-bridge:3100/audio?device_id=test
```

### Áudio com ruído

```bash
# Aumentar noise suppression
vim config/audio.yml
noise_suppression:
  level: very_high

# Ou ajustar AGC
agc:
  target_level: -6  # Reduz ganho
```

### Latência alta

```bash
# Ver métricas
curl http://audio-bridge:3100/metrics | grep audio_bridge_latency

# Reduzir frame size
vad:
  frame_duration_ms: 20  # Menor = mais frequente = maior CPU
```

### Múltiplos tablets dessincronizados

```bash
# Verificar device_id único
docker logs -f audio-bridge | grep device_connected

# Deve mostrar:
# tablet_sala, tablet_quarto, etc
```

---

## 💡 Futuro: Microfones USB pela casa

Quando você adicionar microfones USB conectados ao Orange Pi:

```yaml
# config/audio.yml
sources:
  # WebSocket (tablets)
  - type: websocket
    enabled: true
    port: 3100
    
  # Microfone USB sala
  - type: usb_mic
    enabled: true
    device_id: mic_sala
    alsa_device: "hw:1,0"
    room: sala
    
  # Microfone USB quarto
  - type: usb_mic
    enabled: true
    device_id: mic_quarto
    alsa_device: "hw:2,0"
    room: quarto
    
  # Microfone USB cozinha
  - type: usb_mic
    enabled: true
    device_id: mic_cozinha
    alsa_device: "hw:3,0"
    room: cozinha

# Roteamento inteligente
routing:
  # Resposta vai para o device que fez a pergunta
  response_target: source_device
  
  # Ou broadcast (todos os speakers tocam)
  broadcast_mode: false
```

Todos os microfones (tablet + USB) vão para o mesmo `audio-bridge`, que multiplexa os streams.

---

**Documentação atualizada:** 27/11/2025
