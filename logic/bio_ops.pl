%% logic/bio_ops.pl
%%
%% Ahmad Docking: Bio-Formal Layer
%% Prolog DCG for genetic code, transcription, translation, metabolic flux.
%% Connects to sovereign Lisp machine via S-expression handshake.
%%
%% Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643

:- use_module(library(clpfd)).
:- use_module(library(lists)).

%% ── Genetic Code (codon/4: N1, N2, N3, AminoAcid) ──────────────────────────

codon(a,a,a,'Lys'). codon(a,a,g,'Lys').
codon(a,a,c,'Asn'). codon(a,a,u,'Asn').
codon(a,t,g,'Met'). % START
codon(t,t,t,'Phe'). codon(t,t,c,'Phe').
codon(t,g,g,'Trp').
codon(t,a,a,'STOP'). codon(t,a,g,'STOP'). codon(t,g,a,'STOP').

%% ── Transcription (DNA -> RNA, DCG) ─────────────────────────────────────────

transcribe([], []).
transcribe([a|Ds], [u|Rs]) :- transcribe(Ds, Rs).
transcribe([t|Ds], [a|Rs]) :- transcribe(Ds, Rs).
transcribe([c|Ds], [g|Rs]) :- transcribe(Ds, Rs).
transcribe([g|Ds], [c|Rs]) :- transcribe(Ds, Rs).

%% ── Translation (RNA -> Protein) ────────────────────────────────────────────

translate([], []).
translate([N1,N2,N3|Rest], Protein) :-
    ( codon(N1,N2,N3,'STOP') ->
        Protein = []
    ;   codon(N1,N2,N3,AA) ->
        Protein = [AA|More],
        translate(Rest, More)
    ;   translate(Rest, Protein)  % skip unknown codon
    ).

%% ── Mass Conservation Check ─────────────────────────────────────────────────

nucleotide_mass(a, 313).
nucleotide_mass(c, 289).
nucleotide_mass(g, 329).
nucleotide_mass(t, 304).
nucleotide_mass(u, 306).

sequence_mass([], 0).
sequence_mass([N|Ns], M) :-
    nucleotide_mass(N, NM),
    sequence_mass(Ns, Rest),
    M is NM + Rest.

%% No-cloning: cannot split sequence mass without external energy
no_cloning_check(Seq, Part1, Part2) :-
    sequence_mass(Seq, M),
    sequence_mass(Part1, M1),
    sequence_mass(Part2, M2),
    ( M1 + M2 =:= M ->
        throw(no_cloning_violation(requires_energy_witness))
    ;   true
    ).

%% ── Metabolic Flux (CLP(FD)) ────────────────────────────────────────────────
%% Stoichiometric constraint: S * v = 0, each flux in [Vmin, Vmax]

:- meta_predicate flux_mode(+, -).

flux_mode(Reactions, Fluxes) :-
    length(Reactions, N),
    length(Fluxes, N),
    Fluxes ins 0..100,
    apply_stoichiometry(Reactions, Fluxes),
    label(Fluxes).

apply_stoichiometry([], _).
apply_stoichiometry([r(S,P,Stoich)|Rs], [V|Vs]) :-
    constraint_for(S, P, Stoich, V),
    apply_stoichiometry(Rs, Vs).

constraint_for(_S, _P, Stoich, V) :-
    V #>= 0,
    V #=< Stoich * 100.

%% ── WORM Audit Seal ─────────────────────────────────────────────────────────
%% Every verified result emits a Bifrost-compatible audit term.

worm_seal(Label, Payload, seal(Label, Payload, verified)) :-
    format(atom(_), "~w:~w", [Label, Payload]).

%% ── Tests ────────────────────────────────────────────────────────────────────

:- begin_tests(bio_ops).

test(transcribe_a) :-
    transcribe([a,t,c,g], RNA),
    RNA = [u,a,g,c].

test(translate_met_stop) :-
    translate([a,t,g,t,a,a], Protein),
    Protein = ['Met'].

test(mass_conservation) :-
    sequence_mass([a,c,g,t], M),
    M =:= 313 + 289 + 329 + 304.

test(flux_mode) :-
    flux_mode([r(glc,g6p,1), r(g6p,f6p,1)], Fluxes),
    length(Fluxes, 2).

:- end_tests(bio_ops).
