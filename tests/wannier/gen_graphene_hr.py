# 2 orbitals (C1 pZ, C2 pZ)
num_wann = 2
# 7 Wigner-Seitz cells: (0,0,0) and 6 nearest neighbor cells
R_vecs = [
    (0, 0, 0),
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (1, -1, 0),
    (-1, 1, 0),
]
num_ws_cells = len(R_vecs)

lines = []
lines.append(" created by gen_graphene_hr.py")
lines.append(f" {num_wann}")
lines.append(f" {num_ws_cells}")
lines.append("    " + "    ".join(["1"] * num_ws_cells))

t0 = 0.0 # on-site
t1 = -2.7 # nearest neighbor hopping

for R in R_vecs:
    rx, ry, rz = R
    for i in range(1, num_wann+1):
        for j in range(1, num_wann+1):
            t_r = 0.0
            t_i = 0.0
            
            if rx == 0 and ry == 0 and rz == 0 and i == j:
                t_r = t0
            if rx == 0 and ry == 0 and rz == 0 and i != j:
                t_r = t1
            if rx == 0 and ry == 1 and rz == 0 and i == 2 and j == 1:
                t_r = t1
            if rx == 0 and ry == -1 and rz == 0 and i == 1 and j == 2:
                t_r = t1
            if rx == -1 and ry == 0 and rz == 0 and i == 2 and j == 1:
                t_r = t1
            if rx == 1 and ry == 0 and rz == 0 and i == 1 and j == 2:
                t_r = t1
            
            lines.append(f" {rx:4d} {ry:4d} {rz:4d} {i:4d} {j:4d}    {t_r:10.6f}    {t_i:10.6f}")

with open("/Users/jiangx/Desktop/CrystalCanvas/tests/wannier/graphene_hr.dat", "w") as f:
    f.write("\n".join(lines))
    f.write("\n")
