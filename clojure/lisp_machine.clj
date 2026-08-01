;; ahmad-docking/clojure/lisp_machine.clj
;;
;; Clojure port of the Sovereign Lisp Machine (Rust canonical in src/lisp/).
;; Same semantics: METATRON agent, WORM-sealed WorldDump, heap as persistent vector.
;; Integrates with snapkitty-clojure-lisp-bridge.
;;
;; Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643

(ns ahmad-docking.lisp-machine
  (:require [clojure.string :as str]))

;; Word constructors
(defn nil-word  []        {:tag :nil})
(defn bool-word [v]       {:tag :bool :val v})
(defn int-word  [n]       {:tag :int  :val n})
(defn sym-word  [id]      {:tag :sym  :id  id})
(defn cons-word [car cdr] {:tag :cons :car car :cdr cdr})

;; Symbol table
(defn make-symtab [] (atom {:n2i {} :i2n {} :next 0}))
(defn intern! [tbl name]
  (or (get-in @tbl [:n2i name])
      (let [id (:next @tbl)]
        (swap! tbl #(-> % (assoc-in [:n2i name] id)
                          (assoc-in [:i2n id]   name)
                          (update :next inc)))
        id)))
(defn sym-name [tbl id] (get-in @tbl [:i2n id]))

;; Heap
(defn make-heap [] (atom {:cells [] :free [] :n 0}))
(defn halloc! [h w]
  (swap! h update :n inc)
  (let [free (:free @h)]
    (if (seq free)
      (let [idx (first free)]
        (swap! h #(-> % (assoc-in [:cells idx] w) (update :free rest))) idx)
      (let [idx (count (:cells @h))]
        (swap! h update :cells conj w) idx))))
(defn hget [h idx] (get (:cells @h) idx))

;; Environment
(defn make-env [] (atom {0 {:id 0 :parent nil :bindings {}} :next 1}))
(defn extend-env! [e pid]
  (let [id (:next @e)]
    (swap! e #(-> % (assoc id {:id id :parent pid :bindings {}}) (update :next inc))) id))
(defn bind! [e fid sid val] (swap! e assoc-in [fid :bindings sid] val))
(defn lookup [e fid sid]
  (loop [f fid]
    (when f
      (let [frame (get @e f)]
        (if-let [v (get-in frame [:bindings sid])] v (recur (:parent frame)))))))

;; Tokenizer
(defn tokenize [s]
  (->> (-> s (str/replace "(" " ( ") (str/replace ")" " ) "))
       (str/split #"\s+")
       (remove str/blank?)
       vec))

;; Parser
(defn parse [tokens tbl]
  (let [pos (atom 0)]
    (letfn [(adv [] (let [t (get tokens @pos)] (swap! pos inc) t))
            (peek [] (get tokens @pos))
            (expr []
              (let [t (adv)]
                (cond
                  (= t "(")   (lst)
                  (= t "nil") (nil-word)
                  (= t "true")  (bool-word true)
                  (= t "false") (bool-word false)
                  (re-matches #"-?\d+" t) (int-word (Long/parseLong t))
                  :else (sym-word (intern! tbl t)))))
            (lst []
              (if (= (peek) ")")
                (do (adv) (nil-word))
                (cons-word (expr) (lst))))]
      (expr))))

;; Evaluator
(defn word-seq [w]
  (if (= :nil (:tag w)) []
      (cons (:car w) (word-seq (:cdr w)))))

(defn evaluate [machine w fid]
  (let [{:keys [symtab env heap]} machine]
    (case (:tag w)
      (:nil :bool :int :str) w
      :sym (or (lookup env fid (:id w))
               (throw (ex-info (str "Unbound: " (sym-name symtab (:id w))) {})))
      :cons
      (let [fn-w  (evaluate machine (:car w) fid)
            args  (word-seq (:cdr w))
            fname (when (= :sym (:tag fn-w)) (sym-name symtab (:id fn-w)))]
        (case fname
          "+"     (int-word (reduce + (map #(-> (evaluate machine % fid) :val) args)))
          "-"     (let [[a & r] (map #(-> (evaluate machine % fid) :val) args)]
                    (int-word (reduce - a r)))
          "*"     (int-word (reduce * (map #(-> (evaluate machine % fid) :val) args)))
          "cons"  (let [[a b] (map #(evaluate machine % fid) args)]
                    (cons-word a b))
          "car"   (:car (evaluate machine (first args) fid))
          "cdr"   (:cdr (evaluate machine (first args) fid))
          "quote" (first args)
          "list"  (reduce #(cons-word %2 %1) (nil-word)
                          (reverse (map #(evaluate machine % fid) args)))
          (throw (ex-info (str "Unknown: " fname) {})))))))

;; Machine
(defn make-machine
  ([]      (make-machine "METATRON"))
  ([agent] {:symtab (make-symtab)
            :heap   (make-heap)
            :env    (make-env)
            :tick   (atom 0)
            :agent  agent
            :vault  (atom [])}))

(defn machine-eval! [m src]
  (swap! (:tick m) inc)
  (evaluate m (parse (tokenize src) (:symtab m)) 0))

(defn world-seal [m]
  (let [tick @(:tick m)]
    {:tick tick :agent (:agent m)
     :seal (format "%016x" (hash {:tick tick :agent (:agent m)}))}))

;; REPL
(defn run-repl []
  (let [m (make-machine)]
    (println "Ahmad Docking -- Sovereign Lisp Machine (Clojure)")
    (println (str "Agent: " (:agent m) "  |  Omega = TRUST and CODE"))
    (loop []
      (print "lambda> ") (flush)
      (when-let [line (read-line)]
        (when-not (= line "(quit)")
          (try
            (println "=>" (machine-eval! m line))
            (catch Exception e (println "ERR:" (.getMessage e))))
          (recur))))))
