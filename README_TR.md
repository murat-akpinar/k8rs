# k8rs

**Kubernetes kümenizde neyin bozuk olduğunu, sözlüğe bakmadan anlayacağınız
bir dille söyleyen terminal panosu — ve komutu ezberlemenize gerek kalmadan
düzeltmenizi sağlıyor.**

Tek bir ikili dosya. Kümenize hiçbir şey kurulmuyor. Kendi makinenizde, sizin
kubeconfig'inizle çalışıyor; güven modelinin tamamı bu.

```
 nodes 4/4                                      k8rs                   ctx: kind-k8rs · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS   11 ● 12 ▲│▸ ● default/broken-sigterm                                          12s ago  │
│  RESOURCES         │    The last run on record was stopped, and Kubernetes is restarting it      │
│   workloads        │    (CrashLoopBackOff)                                                       │
│   network          │    container app · 4 restarts · ran for 1s · exit 143 (stopped with         │
│   storage          │    SIGTERM, which is an ordinary shutdown and not an error)                 │
│   config           │    → the container's own log holds a shutdown and not a crash, so check     │
│   cluster          │      the liveness and startup probes, then the pod's events for a resize    │
│  ANALYSIS          │      that restarted it, then the node, where a memory killer such as        │
│   capacity         │      earlyoom sends the same signal                                         │
│   certificates     │                                                                             │
│   drain safety     │  ● default/broken-restarts10                                       35s ago  │
│   posture          │    Container keeps crashing, and each restart waits longer                  │
│   restarts         │    (CrashLoopBackOff)                                                       │
│   waste            │    container flaky · 3 restarts · ran for under a second · exit 1 (the      │
│   versions         │    application's own error)                                                 │
│                    │    → read the last run's log — it holds the last thing written before that  │
│                    │      run ended, from the program or from the shell that started it. The     │
│                    │      command below is what fetches it, using --previous                     │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context kind-k8rs get statefulsets -A --watch                                        │
│ $ kubectl --context kind-k8rs get daemonsets -A --watch                                          │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                                    │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

*Gerçek bir kare: test manifestosu uygulanmış dört düğümlü bir kind kümesine
karşı ikili dosyanın kendisinden alındı — maket değil. **Uygulamanın dili
İngilizcedir**; bu çeviri belgenindir, ekranın değil.*

## Neden

`kubectl get pods` size bir pod'un `CrashLoopBackOff` durumunda olduğunu
söyler. Konteynerin çökmediğini, **durdurulduğunu**; 143 çıkış kodunun sıradan
bir kapanma sinyali olduğunu; sıradaki bakılacak yerin liveness probe olduğunu
söylemez. k8rs, kümenin zaten bildiği şeyi okuyup o kısmı yüksek sesle söyler.

İçinde her yerde geçerli iki kural var:

- **Jargon açıklanır, öylece basılmaz.** `OOMKilled`, *konteyner bellek
  sınırını aştı* olur; özgün sözcük parantez içinde kalır, böylece onu
  aratabilirsiniz.
- **k8rs'in çalıştırdığı her komut, sizin yazacağınız `kubectl` hâliyle
  gösterilir.** Alttaki şerit bu günlüktür. k8rs yazdığı şeyi çalıştırmaz —
  orada olma sebebi komutu öğrenmeniz ve aracı denetleyebilmenizdir.

## Bugün ne yapıyor

- **Alerts** — pod'lar, iş yükleri, düğümler ve kubeconfig'inizin kendi
  sertifikası üzerindeki tüm kontroller, canlı: sürekli çöken bir konteyner,
  zamanlayıcının yerleştiremediği bir pod (zamanlayıcının kendi gerekçesiyle),
  çekilemeyen bir imaj, eksik bir ConfigMap veya Secret, bellek sınırı yüzünden
  öldürülen bir konteyner, çalışan ama Service'inin dışında kalan bir pod,
  süresi dolmak üzere olan bir sertifika ve diğerleri.
- **Analysis** — küme geneli yedi rapor: kapasite, sertifikalar, boşaltma
  güvenliği, duruş, yeniden başlatmalar, israf, sürümler.
- **Düzeltmek** — `r` bir iş yükünü yeniden başlatır, `ctrl-d` bir nesneyi
  siler. Her biri önce sorar, ne olacağını sade bir dille yazan bir kutuyla, ve
  her biri kayda geçer. Bkz.
  *[k8rs kümenizde neyi değiştirebilir](#k8rs-kümenizde-neyi-değiştirebilir)*.
- **`k8rs --once`** — bağlan, tek bir rapor bas, çık: çalıştıysa `0`,
  çalışamadıysa `2`. Terminal veya CI için; yedi rapor için `--analysis` ekleyin.
- **`--read-only`** — değiştiren her tuş yalnızca boşta bırakılmaz, yapısal
  olarak erişilemez olur; böyle bir çalışma hiç denetim günlüğü açmaz, çünkü
  hiçbir şeyi değiştiremeyen bir çalışmanın kaydı da olmaz.

**Henüz bağlanmadı:** **Resources** tarayıcısı kümenizin sunduğu türleri
listeler, ama bir tür açtığınızda boş kalan bir panele düşersiniz — satır
çiziliyor, arkasındaki okuma bağlı değil. Bir kartın arkasındaki dört detay
sekmesi de öyle. İkisi de v0.2'de geliyor.

## Kurulum

k8rs henüz crates.io'da değil — ad, içinde kod olmayan bir yer tutucuyla
ayrılmış durumda, yani bugün `cargo install k8rs` size hiçbir şey getirmez. İlk
sürüme kadar derleyin:

```sh
git clone https://github.com/murat-akpinar/k8rs
cd k8rs
cargo install --path .
```

Rust 1.98.1 veya üstü. Başka bağımlılık yok, kurulacak bir şey de yok.

## Çalıştırmak

```sh
k8rs                            # kubeconfig'teki geçerli bağlam
k8rs --context prod-eu          # adıyla bir bağlam
k8rs --namespace payments       # tek bir namespace (kısası -n)
k8rs --read-only                # hiçbir şey değiştirilemez, yapısal olarak
k8rs --once [--analysis]        # stdout'a tek rapor, sonra çıkış
```

Birden fazla bağlam varsa ve `--context` verilmemişse, k8rs bağlanmadan önce
hangi kümeye gideceğini sorar. `?` bütün tuşları gösterir. `q` çıkar.

## k8rs kümenizde neyi değiştirebilir

Kodda üç işlem var: **scale**, **restart** ve **delete**. Bu yapıda konsol
**restart** ve **delete** sunuyor; `s scale` her tür ve her oturum için geri
çekilmiş durumda — bu yapı hakkında bir gerçek, sizin yetkileriniz hakkında
değil.

İstisnasız hepsinde:

1. Sizin seçtiğiniz bir nesne — toplu işlem diye bir şey yok, seçim olmadan da
   işlem yok.
2. Bir tuşa basış.
3. Sonucu sade bir dille yazan ve karşılığı olan `kubectl` satırını gösteren
   bir onay kutusu.
4. İşlem kabul ediyorsa **sunucu tarafında bir deneme çalıştırması**, yani
   değişiklik yapılmadan önce kümenin kontrol etmesi. Bunu reddeden tek işlem
   `delete`'tir ve bunu hem kendi kutusunda hem denetim günlüğünde söyler.
5. Bir **denetim satırı** — reddettikleriniz de dâhil her deneme,
   `~/.local/state/k8rs/audit.log` içinde (kip `0600`, yalnızca ekleme). Hem
   `kubectl` satırını hem gerçek API çağrısını kaydeder, çünkü k8rs API'yi
   doğrudan çağırır ve ikisi birbirine karıştırılmamalıdır.

**Delete ayrıca nesnenin adını yazmanızı ister.** Eşleşene kadar onay düğmesi
ölüdür.

Onayla gönderim arasında nesne değişir veya ortadan kalkarsa k8rs bunu fark
eder ve hiçbir şey göndermez — az önce geçmiş bir deneme çalıştırması, şu an
için verilmiş bir söz değildir.

## Yetkiler

İki rol var; böylece hangi kipte çalıştığınızı yalnızca araç değil, kümenin
kendisi de zorunlu kılar. İkisi de
[docs/security.md](docs/security.md#rbac) dosyasından alındı; orada her kural,
var olma gerekçesi ve arkasındaki ölçümle birlikte duruyor.

**Salt okunur** — Alerts ve Analysis'in ihtiyaç duyduğu her şey. Bununla
çalıştırdığınızda k8rs'e güvenmeniz gereken bir şey kalmaz:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-readonly
rules:
  - nonResourceURLs: ["/api", "/apis", "/api/*", "/apis/*", "/version"]
    verbs: ["get"]
  - apiGroups: [""]
    resources: ["pods", "pods/log", "events", "services", "nodes",
                "persistentvolumeclaims"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets", "replicasets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["policy"]
    resources: ["poddisruptionbudgets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["certificates.k8s.io"]
    resources: ["certificatesigningrequests"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["discovery.k8s.io"]
    resources: ["endpointslices"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["metrics.k8s.io"]
    resources: ["nodes"]
    verbs: ["get", "list"]
  - apiGroups: ["authorization.k8s.io"]
    resources: ["selfsubjectrulesreviews", "selfsubjectaccessreviews"]
    verbs: ["create"]
```

**İşlemler** — yazma yolunun, salt okunur rolün üstüne ihtiyaç duyduğu şeyler.
Bu rol, v0.2 ve v0.4'te gelecek cordon, drain ve edit fiilleri dâhil işlem
kümesinin tamamını kapsıyor; yalnızca bu yapının yapabildiklerini istiyorsanız
`pods/eviction` ve `nodes` kurallarını çıkarın:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-admin
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["delete"]
  - apiGroups: [""]
    resources: ["pods/eviction"]     # drain
    verbs: ["create"]
  - apiGroups: [""]
    resources: ["nodes"]
    verbs: ["patch", "delete"]       # cordon / uncordon; delete
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets"]
    verbs: ["get", "patch", "update", "delete"] # rollout restart, edit, delete
  - apiGroups: ["apps"]
    resources: ["replicasets"]
    verbs: ["delete"]
  - apiGroups: ["apps"]
    resources:
      ["deployments/scale", "statefulsets/scale", "replicasets/scale"]
    verbs: ["get", "patch"]          # scale
  - apiGroups: ["authorization.k8s.io"]
    resources: ["selfsubjectrulesreviews", "selfsubjectaccessreviews"]
    verbs: ["create"]
```

Bir reddediliş asla çökme ve asla sonsuz bir yeniden deneme değildir: k8rs
rolün hangi fiili ve hangi kaynağı eksik bıraktığını söyler, ve o fiile ihtiyaç
duyan özellik tek başına devre dışı kalır. Küme *bu oturum neler yapabilir*
sorusunu yanıtlayabiliyorsa, kullanamayacağınız tuşlar basmadan **önce** `no`
işaretiyle çizilir.

## Telemetri yok

Makinenizden hiçbir şey çıkmıyor. k8rs'in açtığı tek bağlantı,
kubeconfig'inizin adını verdiği API sunucusuna. Analitik yok, çökme raporlama
yok, güncelleme kontrolü yok — ve bunlardan birini açabilecek bir ayar da yok.

Kimlik bilgileriniz kubeconfig'in geçerli bağlamından okunur, başka hiçbir
yerden: kodda küme içi ServiceAccount yolu diye bir şey yok. TLS doğrulaması
k8rs tarafından asla kapatılmaz; kubeconfig'iniz `insecure-skip-tls-verify`
ayarlıyorsa buna uyulur **ve bu üst satırda söylenir**.

## Ne değil

- **Kuracağınız bir pano değil.** Kümeye hiçbir şey kurulmuyor, sunucu bileşeni
  de yok.
- **Admission controller değil.** Duruş satırları size düğümü devreden bir
  bağlamayı anlatır; kimsenin öyle bir şey oluşturmasını engellemez.
- **YAML düzenleyici değil** — v0.1'de değil. `edit`, kendisine ait fark
  görünümü ve deneme çalıştırmasıyla birlikte v0.4'te geliyor.

## Belgeler

| | |
|---|---|
| Nasıl kurgulandı ve veri akışı | [docs/architecture.md](docs/architecture.md) |
| Güvenlik modeli, RBAC gerekçeleri, denetim günlüğü | [docs/security.md](docs/security.md) |
| Crate'ler, sürümler, derleme hedefleri | [docs/tech-stack.md](docs/tech-stack.md) |
| Her ekran, tuş tuş | [screens/](screens/) |
| Her kararın nedeni, `D1…` diye numaralı | [NOTES.md](NOTES.md) |

English: [README.md](README.md).

## Lisans

GPL-3.0-or-later. Bkz. [LICENSE](LICENSE).
