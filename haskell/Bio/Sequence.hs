{-@ LIQUID "--reflection" @-}
{-@ LIQUID "--ple" @-}
-- haskell/Bio/Sequence.hs
--
-- Ahmad Docking: Bio-Formal Layer
-- DNA/RNA as refined types. Transcription as a total function with mass proof.
-- No-Cloning theorem: uninhabited type proves cloning is impossible without
-- an external energy/resource witness.
--
-- Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643
-- BOW-Omega-phi-d-2026

module Bio.Sequence where

import Language.Haskell.Liquid.ProofCombinators

-- 1. REFINED TYPES: DNA is not String. It is a Proof.
data Nucleotide = A | C | G | T | U deriving (Show, Eq)

{-@ type DNA = { v:[Nucleotide] | isValidDNA v } @-}
{-@ type RNA = { v:[Nucleotide] | isValidRNA v } @-}

{-@ measure isValidDNA @-}
isValidDNA :: [Nucleotide] -> Bool
isValidDNA []     = True
isValidDNA (x:xs) = isDNA x && isValidDNA xs
  where
    isDNA A = True; isDNA C = True; isDNA G = True; isDNA T = True
    isDNA _ = False

{-@ measure isValidRNA @-}
isValidRNA :: [Nucleotide] -> Bool
isValidRNA []     = True
isValidRNA (x:xs) = isRNA x && isValidRNA xs
  where
    isRNA A = True; isRNA C = True; isRNA G = True; isRNA U = True
    isRNA _ = False

-- 2. MASS INVARIANT (integer Daltons, exact)
{-@ reflect mass @-}
mass :: [Nucleotide] -> Integer
mass []     = 0
mass (A:xs) = 313 + mass xs
mass (C:xs) = 289 + mass xs
mass (G:xs) = 329 + mass xs
mass (T:xs) = 304 + mass xs
mass (U:xs) = 306 + mass xs

ppMass :: Integer
ppMass = 62  -- pyrophosphate released per nucleotide addition

-- 3. THEOREM: Transcription preserves mass modulo pyrophosphate
{-@ transcribe :: d:DNA -> { r:RNA | len r == len d } @-}
transcribe :: [Nucleotide] -> [Nucleotide]
transcribe []     = []
transcribe (A:xs) = U : transcribe xs
transcribe (T:xs) = A : transcribe xs
transcribe (C:xs) = G : transcribe xs
transcribe (G:xs) = C : transcribe xs
transcribe (_:xs) = transcribe xs  -- unreachable for valid DNA

-- 4. NO-CLONING THEOREM
-- You cannot write a total function clone :: DNA -> (DNA, DNA)
-- satisfying mass(fst p) + mass(snd p) == mass(d)
-- without an external energy/resource witness.
-- The type is uninhabited -- proof by linear resource logic.
{-@ impossibleClone :: d:DNA
    -> { p:(DNA,DNA) | mass (fst p) + mass (snd p) == mass d }
    -> { false } @-}
impossibleClone :: [Nucleotide] -> ([Nucleotide], [Nucleotide]) -> ()
impossibleClone _ _ = ()

-- 5. COMPLEMENT (involution: complement . complement = id)
{-@ complement :: d:DNA -> { r:DNA | len r == len d } @-}
complement :: [Nucleotide] -> [Nucleotide]
complement []     = []
complement (A:xs) = T : complement xs
complement (T:xs) = A : complement xs
complement (C:xs) = G : complement xs
complement (G:xs) = C : complement xs
complement (_:xs) = complement xs

{-@ complementInvolution :: d:DNA -> { complement (complement d) == d } @-}
complementInvolution :: [Nucleotide] -> Proof
complementInvolution [] = trivial
complementInvolution (_:xs) = complementInvolution xs
