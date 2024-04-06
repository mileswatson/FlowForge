import matplotlib.pyplot as plt
import numpy as np
import json
import argparse

parser = argparse.ArgumentParser("plot_trace")
parser.add_argument("input", help="JSON trace file to plot.", type=str)
args = parser.parse_args()

with open(args.input, "r") as f:
    input = json.load(f)

timestamps = np.array(input["timestamps"])
utilities = np.array(list(map(lambda x: np.nan if x is None else x, input["aggregate_utility"])))
mean = np.mean(utilities[~np.isnan(utilities)])
std = np.std(utilities[~np.isnan(utilities)])
norm_utilities = (utilities - mean) / std

active_senders = np.array(input["active_senders"])

# plot
fig, axes = plt.subplots(len(input["flows"])+2,3)

axes[0][0].plot(timestamps, sum(map(lambda f: np.array(f["bandwidth_kbps"]), input["flows"])))
axes[0][1].plot(timestamps, np.nanmean(list(map(lambda f: np.array(f["rtt_ms"], dtype=float), input["flows"])), axis=0))
axes[0][2].plot(timestamps, utilities)
axes[1][2].plot(timestamps, active_senders)

for i in range(len(input["flows"])):
    flow = input["flows"][i]
    ax = axes[i+2]
    ax[0].plot(timestamps, np.array(flow["bandwidth_kbps"]))
    ax[1].plot(timestamps, np.array(flow["rtt_ms"]))
    ax[2].plot(timestamps, np.array(flow["utility"]))

plt.show()

