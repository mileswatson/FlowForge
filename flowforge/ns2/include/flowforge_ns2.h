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

    extern const RemyDna *load_dna(const char *path);

    extern void free_dna(const RemyDna *dna);

    extern CAction get_action(
        const RemyDna *dna,
        double ack_ewma_ms,
        double send_ewma_ms,
        double rtt_ratio,
        unsigned int current_window);
}
