// Documentation text for the Docs tab.
// The 13 extras text is verbatim from EXTRA_DOCS in extras.py; the 5 core
// field summaries are condensed from docs/algos/core_*.md.

export const EXTRA_LABELS = [
  "Reaction Diffusion",
  "Cellular Automata",
  "Resistor Network",
  "Ising Spin",
  "Wave Resonance",
  "Lattice Boltzmann",
  "Spectral Lowfreq",
  "Hodge Curl",
  "Ant Pheromone",
  "Fuzzy Future",
  "Topo Persistence",
  "Latent Channels",
  "Tensor Kernel",
];

export const EXTRA_KEYS = [
  "reaction_diffusion", "cellular_automata", "resistor_network", "ising_spin",
  "wave_resonance", "lattice_boltzmann", "spectral_lowfreq", "hodge_curl",
  "ant_pheromone", "fuzzy_future", "topo_persistence", "latent_channels",
  "tensor_kernel",
];

export const CORE_KEYS = [
  "core_pressure", "core_attack", "core_resistance", "core_trace", "core_flow",
];
export const CORE_LABELS = {
  core_pressure: "Pressure (P)",
  core_attack: "Attack (A)",
  core_resistance: "Resistance (R)",
  core_trace: "Trace (T)",
  core_flow: "Flow (F)",
};

export const DOCS = {
  // ---- Core fields (condensed from docs/algos/core_*.md) ------------------
  core_pressure:
    "Pressure Field (P = F_w - F_b):\n" +
    "The big-picture altitude map of the board, with two weather systems\n" +
    "fighting for control. Compute forward fields for both sides, subtract,\n" +
    "and squash with tanh.\n\n" +
    "P = F_w - F_b\ntanh_p = tanh(beta_p * P)\nctrl = sum(center_weight * tanh_p)",
  core_attack:
    "Attack Map (A):\n" +
    "The spotlight of immediate reach. Each piece marks the squares it can\n" +
    "hit, weighted by piece strength and a center bias. Feeds safety, capture\n" +
    "logic and the flow source term.\n\n" +
    "for piece: for square in attacks(piece): A[square] += weight[type]",
  core_resistance:
    "Resistance Field (R):\n" +
    "Friction. Occupancy, enemy attack, pawn locks and king zones stack into\n" +
    "R, used as a damping factor 1/(1+R) so influence leaks slowly through\n" +
    "risky or blocked squares.\n\n" +
    "damp = 1 / (1 + R)\nF_next = diffuse(F) * damp + source",
  core_trace:
    "Trace Field (T):\n" +
    "Memory with momentum. Deposit a trail along the last move, project a soft\n" +
    "glow in the forward (quaternion) direction, and decay it over time.\n\n" +
    "T[0]  += trace_past * path\nT[t] += trace_future * exp(-dist/sigma) * align",
  core_flow:
    "Flow Propagation (F):\n" +
    "The engine room. Build a source S from attack + trace + pressure\n" +
    "alignment, diffuse it spatially, damp by resistance, and relax to a\n" +
    "steady state across time.\n\n" +
    "S = A + T + trace_align * tanh(beta_p * P)\nF[t+1] = gamma * diffuse(F[t]) * damp + S[t+1]",

  // ---- Extras (verbatim from extras.py EXTRA_DOCS) ------------------------
  reaction_diffusion:
    "Reaction-Diffusion (Gray-Scott):\n" +
    "Two chemicals play tag. U feeds V, V eats U, and both diffuse.\n" +
    "U_t = du*Lap(U) - U*V^2 + f*(1-U)\n" +
    "V_t = dv*Lap(V) + U*V^2 - (f+k)*V\n" +
    "We paint the V pattern. If it blooms near the enemy king, good.",
  cellular_automata:
    "Cellular Automata:\n" +
    "Pressure becomes a binary grid, then Life-like rules march on.\n" +
    "Alive survives with 2 neighbors, birth with 3 neighbors.\n" +
    "We reward stable colonies that crawl into the enemy king zone.",
  resistor_network:
    "Resistor Network:\n" +
    "The board is a circuit. Squares are nodes with resistance R.\n" +
    "Pieces are voltage sources. Relax: V = (avg + source)/(1+R).\n" +
    "High voltage near the enemy king means strong control.",
  ising_spin:
    "Ising Spin Lattice:\n" +
    "Spins want to align, but pressure pushes them around.\n" +
    "s = tanh(beta*(J*sum_neighbors + field))\n" +
    "Field is mean pressure. High spin near enemy king is rewarded.",
  wave_resonance:
    "Wave Resonance:\n" +
    "We ring the board like a drum, with damping.\n" +
    "u_{t+1}=(2-d)u_t-(1-d)u_{t-1}+c^2*Lap(u)+source\n" +
    "Energy is |u|. Loud waves near the enemy king are good.",
  lattice_boltzmann:
    "Lattice-Boltzmann-like flow:\n" +
    "We push a density rho along pressure gradients.\n" +
    "rho <- rho + advect + diffusion, then damp by occupancy.\n" +
    "High rho near the enemy king means the flow favors us.",
  spectral_lowfreq:
    "Spectral Low-Frequency:\n" +
    "Repeated smoothing strips away noise, leaving global structure.\n" +
    "We reward low-frequency pressure that leans into the enemy king.",
  hodge_curl:
    "Hodge / Curl Proxy:\n" +
    "Curl is a fancy word for 'spinning flow'. We use |Lap(P)|.\n" +
    "High curl near the enemy king suggests trap-like dynamics.",
  ant_pheromone:
    "Ant Pheromone:\n" +
    "Pheromone diffuses and decays: P <- 0.85*P + 0.15*smooth(P).\n" +
    "Pieces seed it by value. High pheromone near enemy king is tasty.",
  fuzzy_future:
    "Fuzzy Future:\n" +
    "Each piece smears into a Gaussian cloud. Sum = possibility field.\n" +
    "We like clouds near the enemy king and hovering over enemy pieces.",
  topo_persistence:
    "Topo Persistence (rough):\n" +
    "Threshold pressure into positive and negative islands.\n" +
    "Count islands touching king zones. Positive near enemy is good;\n" +
    "negative near us is bad. Think of it as persistent threats.",
  latent_channels:
    "Latent Channels:\n" +
    "Three hidden signals:\n" +
    "h0=tanh(pressure+history), h1=tanh(|grad P|),\n" +
    "h2=tanh(trace_w-trace_b). Latent = h0+h1+h2.\n" +
    "We reward latent mass near the enemy king. Vibes, but correct.",
  tensor_kernel:
    "Tensor Kernel:\n" +
    "Each piece stamps a tiny kernel K at its square.\n" +
    "We sum signed kernels into a hidden channel.\n" +
    "High kernel mass near the enemy king is good.",
};
