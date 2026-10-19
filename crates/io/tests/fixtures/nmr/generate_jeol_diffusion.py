"""Original synthetic JDF v1: 14 gradients, four complex points per increment.

Uses the same documented test layout as generate_jeol_nus.py. All timing
records are float64 milliseconds, with no numeric scaling. The pulse section
is deliberately original and must never qualify for automatic confirmation.
No experimental samples or vendor pulse-program text are included.
"""
from pathlib import Path
import struct

times_ms = {
    "delta": 3.5, "delta_large": 100.0, "tau_1": 1.0, "tau_2": 1.0,
    "diffusion_time": 100.0, "grad_1": 1.75, "tau_e1": 2.75,
    "tau_e2": 2.75, "st_coef": 96.66666666666667,
}
records = [(name, 2, value) for name, value in times_ms.items()]
records += [("experiment", 0, "bpp_dste_led_dif"),
            ("grad_shape", 0, "SQUARE"), ("x_domain", 0, "1H")]
parameter_length = 16 + 64 * len(records)
data_start = 1360 + parameter_length
# SmallTwoD tiles have edge 4: fourteen rows occupy sixteen storage rows.
data_length = 2 * 16 * 4 * 8
blob = bytearray(data_start + data_length)
blob[:8] = b"JEOL.NMR"
blob[8:15] = bytes([1, 1, 0, 2, 2, 3, 12])
blob[24:26] = bytes([3, 1])  # complex direct, real parameter axis
blob[32:36] = bytes([1, 28, 1, 31])
for offset, value in [(176, 4), (180, 16), (240, 3), (244, 13),
                      (1212, 1360), (1216, parameter_length), (1284, data_start)]:
    struct.pack_into(">I", blob, offset, value)
struct.pack_into(">Q", blob, 1288, data_length)
struct.pack_into(">d", blob, 336, 0.003)
struct.pack_into("<4I", blob, 1360, 64, 0, len(records), parameter_length)
for index, (name, kind, value) in enumerate(records):
    offset = 1376 + index * 64
    if kind == 2:
        blob[offset + 6:offset + 8] = bytes([0x11, 28])  # ms, power 1
        struct.pack_into("<d", blob, offset + 16, value)
    else:
        blob[offset + 16:offset + 16 + len(value)] = value.encode("ascii")
    struct.pack_into("<i", blob, offset + 32, kind)
    blob[offset + 36:offset + 36 + len(name)] = name.encode("ascii")
for row in range(14):
    for column in range(4):
        struct.pack_into("<d", blob, data_start + (row * 4 + column) * 8,
                         1.0 + row + column / 10.0)
blob += ("\ng => y_acq {" + ", ".join(f"{20 * i}[mT/m]" for i in range(1, 15))
         + "};\noriginal_synthetic_diffusion.jxp\npulse\n"
         + "  synthetic_timing_marker;\nend pulse;\n").encode("ascii")
Path(__file__).with_name("jeol-diffusion.jdf").write_bytes(blob)
