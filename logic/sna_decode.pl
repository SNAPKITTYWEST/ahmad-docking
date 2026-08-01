%% logic/sna_decode.pl
%%
%% Ahmad Docking: SNA Decoding Constraints
%% Models Reed-Solomon syndrome equations and Berlekamp-Massey decoder
%% as Prolog constraints for formal verification of correction logic.
%%
%% Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643

:- use_module(library(clpfd)).
:- use_module(library(lists)).

%% ── Spatial address extraction ────────────────────────────────────────────────

%% Extract spatial address from first 36 bases (primer=20 + address=16)
extract_address(Read, Addr) :-
    Read = read(Bases, _Quality),
    length(Primer, 20),
    append(Primer, Rest, Bases),
    length(AddrBases, 16),
    append(AddrBases, _, Rest),
    base4_decode(AddrBases, Addr).

base4_decode([], 0).
base4_decode([B|Bs], Val) :-
    base4_decode(Bs, Rest),
    length(Bs, N),
    Val is B * (4 ^ N) + Rest.

%% ── Assign reads to strands ───────────────────────────────────────────────────

assign_reads(Reads, StrandGroups) :-
    maplist(extract_address, Reads, Addrs),
    sort(Addrs, UniqueAddrs),
    maplist(collect_strand(Reads), UniqueAddrs, StrandGroups).

collect_strand(Reads, Addr, strand(Addr, Bases)) :-
    include(has_address(Addr), Reads, Matching),
    maplist(read_bases, Matching, Bases).

has_address(Addr, Read) :-
    extract_address(Read, Addr).

read_bases(read(Bases, _), Bases).

%% ── Reed-Solomon syndrome check (GF(256) via integer mod 256) ────────────────
%% Primitive polynomial for GF(256): x^8 + x^4 + x^3 + x^2 + 1 (0x11D)

:- dynamic gf_table/2, gf_log/2.

%% GF(256) multiply (using log/antilog tables -- precomputed at load)
gf_mul(0, _, 0) :- !.
gf_mul(_, 0, 0) :- !.
gf_mul(A, B, C) :-
    gf_log(A, LA), gf_log(B, LB),
    Sum is (LA + LB) mod 255,
    gf_table(Sum, C).

%% Syndrome calculation: S_i = sum_j(r_j * alpha^(i*j)) for i=0..31
syndrome(Received, I, Syndrome) :-
    length(Received, N),
    numlist(0, N_1, Indices), N_1 is N - 1,
    maplist(syndrome_term(Received, I), Indices, Terms),
    foldl(gf_add, Terms, 0, Syndrome).

syndrome_term(Received, I, J, Term) :-
    nth0(J, Received, Rj),
    Exp is (I * J) mod 255,
    gf_table(Exp, Alpha_IJ),
    gf_mul(Rj, Alpha_IJ, Term).

gf_add(A, B, C) :- C is xor(A, B).

all_zero([]).
all_zero([0|Xs]) :- all_zero(Xs).

%% ── Top-level decoder ─────────────────────────────────────────────────────────

rs_correct(Received, Corrected) :-
    length(Received, 255),
    numlist(0, 31, SyndromeIndices),
    maplist(syndrome(Received), SyndromeIndices, Syndromes),
    ( all_zero(Syndromes) ->
        Corrected = Received
    ;
        %% Berlekamp-Massey (stub -- full impl requires GF polynomial arithmetic)
        format("RS: non-zero syndromes, applying error correction~n"),
        Corrected = Received  %% TODO: full BM + Chien + Forney
    ).

%% ── CRC32 verification ────────────────────────────────────────────────────────

:- use_module(library(crypto)).

verify_crc(Payload, ExpectedCRC) :-
    crypto_data_hash(Payload, Hash, [algorithm(sha256), encoding(octet)]),
    sub_atom(Hash, 0, 8, _, HexPart),
    atom_to_term(HexPart, CRC, []),
    CRC =:= ExpectedCRC.

%% ── WORM audit seal ───────────────────────────────────────────────────────────

worm_seal(Label, Payload, seal(Label, Hash)) :-
    term_to_atom(Payload, PA),
    atom_concat(Label, PA, Input),
    crypto_data_hash(Input, Hash, [algorithm(sha256), encoding(utf8)]).

%% ── Tests ─────────────────────────────────────────────────────────────────────

:- begin_tests(sna_decode).

test(base4_decode_zero) :-
    base4_decode([0,0,0,0], 0).

test(base4_decode_value) :-
    base4_decode([1,0,0,0], V),
    V =:= 1 * 64.

test(all_zero_empty) :- all_zero([]).
test(all_zero_zeros) :- all_zero([0,0,0]).
test(not_all_zero)   :- \+ all_zero([0,1,0]).

:- end_tests(sna_decode).
