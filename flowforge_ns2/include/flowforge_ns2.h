#pragma once

#include <memory>

struct RemyDna;

extern "C"
{

    typedef struct CAction
    {
        unsigned int new_window;
        double intersend_seconds;
    };

    extern void free_dna(RemyDna *);

    extern const RemyDna *load_dna(const char *path);

    extern CAction get_action(
        RemyDna *dna,
        double ack_ewma_ms,
        double send_ewma_ms,
        double rtt_ratio,
        unsigned int current_window);
}
