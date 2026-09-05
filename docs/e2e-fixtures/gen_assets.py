import wave, struct, csv, os

d = r"d:\Develop\SourceCode\rustcode\LarkReader\_test_fixtures"


def make_wav(p, secs=2):
    n = 44100
    w = wave.open(p, "w")
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(n)
    fr = bytearray()
    for i in range(int(n * secs)):
        v = int(32767 * 0.3 * ((i % 2000) / 1000))
        fr += struct.pack("<h", v)
    w.writeframes(bytes(fr))
    w.close()


make_wav(os.path.join(d, "sample_audio.wav"))

with open(os.path.join(d, "员工数据.csv"), "w", encoding="utf-8-sig", newline="") as f:
    c = csv.writer(f)
    c.writerow(["姓名", "部门", "工号", "入职日期"])
    for r in [
        ["张三", "技术部", "A001", "2021-03-01"],
        ["李四", "产品部", "A002", "2022-07-15"],
        ["王五", "设计部", "A003", "2023-01-20"],
    ]:
        c.writerow(r)


def make_pdf(p, title, body):
    objs = [b"<</Type/Catalog/Pages 2 0 R>>", b"<</Type/Pages/Kids[3 0 R]/Count 1>>"]
    stream = (
        "BT /F1 14 Tf 50 780 Td (%s) Tj ET\nBT /F1 11 Tf 50 750 Td (%s) Tj ET" % (title, body)
    ).encode("latin-1")
    objs.append(
        b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 595 842]/Resources<</Font<</F1 5 0 R>>>>/Contents 4 0 R>>"
    )
    objs.append(
        b"<</Length " + str(len(stream)).encode() + b">>\nstream\n" + stream + b"\nendstream>"
    )
    objs.append(b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>")
    pdf = b"%PDF-1.4\n"
    offs = []
    for i, o in enumerate(objs, 1):
        offs.append(len(pdf))
        pdf += ("%d 0 obj\n" % i).encode() + o + b"\nendobj\n"
    xr = b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for o in offs:
        xr += ("%010d 00000 n \n" % o).encode()
    xr += b"trailer\n<</Size %d/Root 1 0 R>>\nstartxref\n%d\n%%%%EOF" % (len(objs) + 1, len(pdf))
    pdf += xr
    open(p, "wb").write(pdf)


make_pdf(
    os.path.join(d, "test_attachment.pdf"),
    "LarkReader E2E Attachment Test",
    "This PDF is used to verify file-block export behavior. (Chinese: test)",
)

print("done", sorted(os.listdir(d)))
