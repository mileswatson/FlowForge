import matplotlib.pyplot as plt
import numpy as np
import json
import argparse

parser = argparse.ArgumentParser("plot_trace")
parser.add_argument("input", help="JSON train file to plot.", type=str)
args = parser.parse_args()

with open(args.input, "r") as f:
    input = json.load(f)

timestamps = np.array(input["timestamps"][1:])
utility = np.array(input["utility"][1:])
bandwidth = np.array(input["bandwidth"][1:])
rtt = np.array(input["rtt"][1:])

# plot
fig, axes = plt.subplots(4)

axes[0].plot(timestamps, utility)
axes[1].plot(timestamps, np.exp((utility - utility.min())*10))
axes[2].plot(timestamps, bandwidth)
axes[3].plot(timestamps, 1/rtt)

plt.show()

