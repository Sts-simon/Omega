## 0.45.0 — Contrats et progression

- **Contrats détaillés.** Chaque offre porte désormais un contexte fictif, une famille de mission et de véhicule, une difficulté, un budget de conception, une échéance, une récompense et les sections demandées du dossier technique. Les anciennes sauvegardes restent lisibles : les nouveaux champs ont une valeur par défaut et une offre sans budget ou sans objectif conserve son ancien mode de livraison.
- **Objectifs combinés.** Trois objectifs principaux (masse maximale, fiabilité minimale, complexité maximale) sont évalués avec `DesignMetrics` avant la remise. Les objectifs secondaires, optionnels, sont vérifiés séparément : les remplir tous ajoute un bonus à la récompense calculée sur la qualité du dossier. Un budget dépassé, un objectif principal manqué ou un dossier incomplet refuse la livraison et laisse le contrat actif.
- **Panneau de contrat.** La carte d'offre affiche le contexte, le budget, les objectifs et le bonus ; le dossier actif montre les valeurs courantes, l'état de chaque objectif, le budget consommé, la version à soumettre et les sections demandées. Le panneau des livraisons conserve la révision soumise et le bonus payé.
- **Recherche lisible.** Les six départements deviennent des parcours de paliers : chaque colonne affiche sa progression, une ligne `PARCOURS` avec les dépendances, et chaque carte indique son palier, ses prérequis (`OK` ou `requis`), les pièces et projets réellement débloqués, ainsi qu'un repère `NOUVELLE POSSIBILITÉ` pour les technologies racines disponibles. Un bandeau compare les technologies disponibles par coût, durée et déblocage, sans ajouter de technologies artificielles.
- **Révisions de conception.** Le pied de l'atelier liste la tête de version, les deux dernières révisions, leur nombre d'essais associés et la révision soumise. Une révision peut être nommée au clavier ou par bouton, comparée à la tête avec le diff existant, et restaurée ; la restauration crée une nouvelle révision `Restored from …` au lieu de réécrire l'historique. La révision remise est enregistrée dans `CompletedContract.revision_id` et marquée dans le journal.
- **Dossier technique de livraison.** Une remise réussie écrit `contrat-<id>-dossier-<hash8>.pdf` dans `saves/slot_N/reports/`. Le document reprend le contexte et les objectifs, la vue générale, l'arbre d'assemblage, les équipements, les paramètres de conception, le programme embarqué, les campagnes d'essais dont les empreintes correspondent, les anomalies éventuelles, la matrice des exigences et la révision soumise.
- **Compatibilité et vérifications.** Le schéma du véhicule reste 7, le journal de révisions reste 3 ; `CompletedContract` accepte les anciennes livraisons via des champs optionnels. Nouveaux tests sur les objectifs combinés, le budget, le bonus, le refus sans perte de contrat, le nommage/la comparaison/la restauration de révision, la révision soumise, les paliers de recherche, les aperçus de pièces et les sections du dossier PDF. `cargo fmt --all -- --check`, `cargo check --workspace`, clippy avec warnings refusés et les **391 tests** du workspace passent (149 application, 18 `omega-data`, 153 `omega-rules`, 71 `omega-sim-core`). L'interface native n'a pas été ouverte dans cette passe : la nouvelle largeur des cartes de contrat, le bandeau de recherche, le pied de l'atelier et le PDF de livraison restent à vérifier visuellement.

## 0.44.0 — Laboratoire d'essais

- **Tableau de bord.** L'onglet ESSAIS devient un laboratoire à trois pages. Le tableau de bord regroupe le résumé du tir ou du lot, les résultats par phase (durée, vitesse maximale, apogée, masse moyenne), les quatre courbes de trajectoire, les événements horodatés, les statistiques de campagne, la conformité au contrat et les anomalies. Les états vides distinguent l'absence d'essai, le lot en cours et le lot devenu obsolète.
- **Comparaison A/B/C.** La comparaison existante est enrichie : l'instantané A, la conception courante B et la dernière campagne C sont présentés sur la masse, le coût fictif, la stabilité, l'autorité, la complexité et la fiabilité, puis sur la portée, la vitesse, le temps de vol, les décollages manqués, les séparations et les anomalies. Chaque ligne affiche les valeurs absolues, l'écart B−A et la variation en pourcentage ; une base nulle reste `—`. La conformité sur papier de A, B et C est comptée séparément.
- **Historique persistant.** `TrialHistory` enregistre chaque tir et chaque lot dans `trials/history.json`, avec la révision de conception, la version du programme, les graines exactes, le profil de lancement, les empreintes et les instantanés complets du véhicule et du programme. L'onglet HISTORIQUE permet de sélectionner une campagne, de lire ses phases, ses hashes, ses anomalies et son dossier de conformité, puis de la relancer. Une campagne est marquée obsolète dès que la conception ou le programme courant change ; revenir aux entrées exactes la remet à jour.
- **Analyse des anomalies.** Chaque anomalie est classée en instabilité, défaillance de composant, ressource insuffisante, programme incompatible, exigence contractuelle, délai ou autre. Quand l'ancre est connue, un bouton sélectionne la pièce dans l'atelier ou le bloc dans l'éditeur de programme. Les échecs de validation produisent aussi une liste structurée avec la pièce, le bloc et la graine concernés.
- **Exports.** `HTML` produit un rapport autonome avec styles, cartes, graphiques SVG intégrés, phases, anomalies, conformité et tableau de l'historique avec empreintes. `CSV` exporte les échantillons et leurs colonnes de vol. `PDF` produit un dossier technique sans dépendance externe, avec en-tête, résumé, conception, phases, histogramme de trajectoire et résumé des modifications entre révisions. Les trois fichiers sont écrits dans `saves/slot_N/reports/` et n'altèrent pas la campagne.
- **Déterminisme et invalidation.** Les instantanés stockés contiennent tout ce qui a volé ; une relance ignore la conception courante et rejoue le même véhicule, le même programme et la même graine. Le layout visuel du programme reste exclu des empreintes via `Program::semantic_copy()`, donc déplacer une carte n'invalide pas une preuve. Modifier une pièce ou un bloc rend les campagnes et les lots concernés obsolètes.
- **Compatibilité.** Le schéma du véhicule reste 7 et les sauvegardes existantes se chargent sans migration. L'historique possède son propre schéma et son propre fichier, recréé au chargement d'un slot ; les exports sont de nouveaux fichiers, sans changement du format de campagne.
- **Vérifications.** Nouveaux tests sur la comparaison avec pourcentages, les anomalies contractuelles, les phases, les ancres d'anomalie, la relance exacte, l'obsolescence du tir courant, les graines d'un lot, l'aller-retour JSON de l'historique, les exports et le PDF. `cargo fmt --all -- --check`, `cargo check --workspace`, clippy avec warnings refusés et les **385 tests** du workspace passent (144 application, 18 `omega-data`, 152 `omega-rules`, 71 `omega-sim-core`). L'interface native n'a pas été ouverte dans cette passe : la disposition des trois onglets, les tableaux à 1280 × 720 et les rapports exportés restent à vérifier visuellement.

## 0.43.0 — Programmation visuelle avancée

- **Canevas de programmation.** Le programme quitte la liste de chaînes pour un canevas absolu : chaque bloc de premier niveau se déplace librement, le fond se déplace au glisser (bouton gauche ou droit) et la molette règle le zoom (50 % à 200 %). Les positions, le zoom et le déplacement sont enregistrés dans `ProgramLayout` ; les blocs sans position reçoivent une position de colonne par tâche. Un bouton `RESET VIEW` retrouve le cadrage initial, `ALIGN` replace toutes les tâches sur des rails réguliers.
- **Connexions et groupes.** Les blocs consécutifs d'une tâche sont reliés par un tracé en équerre ; les blocs de contrôle affichent leurs branches et leurs blocs imbriqués directement dans la carte. Les groupes nommés dessinent une étiquette et un liseré violet autour de leurs membres.
- **Sélection multiple et édition en lot.** Le mode `MULTI` ajoute ou retire des blocs au clic. `COPY`, `PASTE`, `DUPLICATE` et `DELETE` travaillent sur la sélection ou sur le bloc primaire ; `GROUP` capture la sélection, `UNGROUP` efface les groupes. Les actions restent disponibles dans le panneau de la sélection et dans la bibliothèque.
- **Structures de contrôle.** `IF / ELSE`, `WAIT UNTIL`, `REPEAT N`, `REPEAT WHILE`, `BREAK` et `CALL ROUTINE` sont des blocs récursifs avec corps `Vec<Block>`. Les branches THEN, ELSE et BODY sont éditables depuis la carte : sélectionner une branche puis ajouter un bloc depuis la bibliothèque l'insère au bon endroit. Les conditions disponibles sont vitesse, pression, drapeau et mode, avec un délai borné.
- **Sous-programmes.** Une sélection peut être capturée en routine nommée ; la bibliothèque de routines liste leur taille, permet de les appeler depuis n'importe quelle tâche et de les supprimer. `CALL ROUTINE` accepte plusieurs appels, les appels inconnus, les doublons de nom et les cycles sont refusés, et la profondeur d'appel est plafonnée à 8. Les boucles sont bornées à 1 024 itérations et `REPEAT WHILE` possède un délai propre.
- **Débogueur visuel.** `RUN` produit une trace et lance la lecture ; les cartes sont cyan pour le bloc courant, vertes pour les blocs terminés, jaunes pour les blocs en attente et rouges pour une anomalie. `PAUSE`/`PLAY`, `< STEP`, `STEP >`, la vitesse 0,5× à 4× et `RESET` pilotent la lecture. Le panneau de sélection affiche le drapeau et le mode courants, le panneau de validation rappelle les drapeaux finaux ; la liste de trace marque chaque étape avec `>`, `+`, `!` ou `·`.
- **Validation en temps réel.** Les erreurs et avertissements restent séparés dans le panneau de droite et sont recalculés à chaque édition : matériel absent, liaisons invalides (`LOCK TARGET`, étages séparés), paramètres hors bornes, délais incohérents (0 s), mémoire et tâches dépassées, drapeaux ou modes jamais écrits, conditions mortes, appels inconnus dans les branches et routines, routines vides ou dupliquées. Les routines comptent dans la mémoire de l'ordinateur.
- **Compatibilité et déterminisme.** `Program` gagne `routines` et `layout`, tous deux `#[serde(default)]` : les anciens JSON se chargent avec des routines vides et un layout neutre. Les anciens programmes sans branches continuent de voler avec le même interpréteur ; les nouveaux blocs produisent des traces et des résultats déterministes. Les empreintes de contenu, les essais et les dossiers ignorent le layout via `Program::semantic_copy()` : déplacer une carte ne rend pas une preuve obsolète.
- **Vérifications.** Nouveaux tests sur les branches, les boucles, `BREAK`, `WAIT UNTIL`, les routines appelées plusieurs fois, la désérialisation des anciens programmes, les compteurs récursifs, les appels inconnus imbriqués, les actions multi-sélection, la capture de routine, le copier/coller, l'alignement et la lecture du débogueur. `cargo fmt --all -- --check`, `cargo check --workspace`, clippy avec warnings refusés et les **375 tests** du workspace passent (134 application, 18 `omega-data`, 152 `omega-rules`, 71 `omega-sim-core`). L'interface native n'a pas été ouverte dans cette passe : le rendu du canevas, le glisser-déposer et le zoom restent à vérifier visuellement à 1600 × 960 et en fenêtre étroite.

## 0.42.0 — Construction 3D avancée

- **Accrochage visuel.** Le glisser-déposer détecte automatiquement les points libres, choisit le plus proche dans un rayon généreux et affiche un fantôme transparent de la pièce à sa taille réelle. Le fantôme suit l'orientation du montage (une surface reste tangente au corps), les marqueurs valides passent au vert, les montages incompatibles au rouge, et le relâchement sur un point rouge est refusé. Les huit montages radiaux sont proposés par les corps (les quatre premiers gardent leur place).
- **Symétrie avancée.** Trois modes : radiale (2 à 8 copies, `X`), bilatérale et axiale (`Maj+X`). Une copie reçoit sa position sur le montage miroir ; les réglages de paramètres, les choix de conception, les rotations et les translations de gizmo sont propagés à tout le groupe. `LIER LA SÉLECTION` réassocie des pièces existantes, `ROMPRE` détache une copie, et supprimer une copie retire le groupe complet. Les transformations de chaque copie sont mises à jour avec le miroir ou la rotation propre au mode, pas recopiées à l'identique.
- **Sous-ensembles.** Le bouton `DUPLIQUER` clone une sélection avec ses liaisons internes sur des points libres du même parent ; déplacer un ancêtre emmène toujours ses enfants. La bibliothèque capture l'arbre sélectionné par catégorie (surfaces, électronique, structure, étage, configuration), le réinsère en rejouant chaque liaison et permet de l'exporter en JSON puis de le réimporter dans une autre conception.
- **Contrôle de l'assemblage.** Un panneau dédié classe les problèmes en connexions, points d'attache occupés, incompatibilités, verrous et sous-ensembles mal connectés. Chaque ligne nomme la pièce et la sélectionne d'un clic, dans l'arbre comme en 3D.
- **Compatibilité.** Le schéma véhicule reste 7 : le mode et l'index de symétrie sont encodés dans l'identifiant de groupe existant (`sym-<n>-<mode><index>`), les anciens groupes `sym-<n>` se lisent comme radiaux. La bibliothèque utilise son propre fichier versionné (`designs/subassemblies.json`) et les exports vivent dans `exports/subassemblies/`. Les sauvegardes et les preuves existantes restent lisibles.
- **Vérifications.** Nouveaux tests sur les huit montages, le décodage des groupes, les miroirs, la propagation d'un réglage et d'une transformation, la réassociation, la rupture de lien, la duplication d'un sous-arbre, l'aller-retour bibliothèque et le rapport d'assemblage. `cargo fmt --all -- --check`, `cargo check --workspace`, clippy avec warnings refusés et les 366 tests du workspace passent (131 application, 18 `omega-data`, 146 `omega-rules`, 71 `omega-sim-core`). L'interface native n'a pas été ouverte dans cette passe : l'aperçu, les couleurs et la bibliothèque restent à vérifier visuellement à 1600 × 960 et en fenêtre étroite.

## 0.41.0 — Ergonomie générale de l'atelier

- **Sélection multiple et surbrillance.** Le survol dessine un cadre cyan autour de la pièce ; la sélection est en ambre, en rouge pour une pièce verrouillée. `Ctrl+clic` ajoute ou retire une pièce, `Échap` désélectionne sans quitter l'atelier, et la sélection se fait indifféremment en 3D ou dans l'arbre. Cliquer sur un membre d'une sélection multiple conserve le groupe et change seulement la pièce primaire du gizmo. Le déplacement applique le même delta local à chaque racine sélectionnée indépendante : aucun sous-arbre n'est déplacé deux fois et la hiérarchie reste intacte.
- **Arbre d'assemblage outillé.** Les groupes se plient et se déplient, affichent leur nombre de pièces et leur étage, se renomment ; un bouton sélectionne tout le sous-ensemble, un autre le masque temporairement (la 3D et le picking l'ignorent, les règles continuent de le compter) et un verrou protège la pièce et ses enfants. Supprimer ou déplacer un ancêtre contenant une pièce verrouillée est refusé au lieu d'emporter silencieusement la pièce protégée.
- **Caméra complète.** `F` cadre la sélection (y compris plusieurs pièces), `Début` cadre le véhicule, `1` à `5` donnent les vues face, côté, dessus, arrière et isométrique. Les cadrages passent par un but interpolé, donc la transition est douce. `V` bascule entre caméra orbitale et libre, `[` / `]` règlent la vitesse, et les mêmes commandes sont disponibles en boutons dans la palette.
- **Palette enrichie.** Recherche par nom ou description avec `/` (ou le champ de la palette), filtres de catégorie, favoris avec `+`/`*`, pièces récemment utilisées, aperçu 3D tournant rendu dans une texture dédiée, description au survol et technologie encore nécessaire. Le panneau rappelle les commandes souris et clavier.
- **Aide contextuelle.** L'inspecteur présente le rôle de la pièce, les conséquences abstraites de ses paramètres, l'incompatibilité la plus facile à manquer et la recherche manquante, à côté des contrôles de verrouillage, masquage, sélection de groupe et renommage.
- **État d'interface hors sauvegarde.** Sélection, plis, masquages, verrous, noms de groupe, favoris, récents et recherche vivent dans `MissileSession` et ne sont pas sérialisés : le schéma véhicule reste 7 et les anciennes sauvegardes se chargent sans migration. Les actions purement visuelles ne réécrivent plus `designs/vehicle.json` ; le fichier n'est enregistré que lorsque le véhicule change réellement.
- **Vérifications.** Six tests ajoutés (sélection multiple, sélection de groupe, pliage/renommage, recherche de palette, verrou d'un enfant protégeant son ancêtre, déplacement de plusieurs racines), soit 352 tests workspace (122 application, 18 `omega-data`, 141 `omega-rules`, 71 `omega-sim-core`). `cargo fmt --all -- --check`, `cargo check --workspace`, clippy avec warnings refusés et les tests passent. L'interface native n'a pas été capturée sur écran dans cette passe ; les panneaux restent à vérifier visuellement à 1600 × 960 et sur une fenêtre plus étroite.

## 0.40.1 — Diamètres de montage et coiffe joueur

- **Gamme de diamètre pour les pièces à section fixe.** Les capteurs, ordinateurs, séparateurs, modules de mission, charges et modules d'avionique exposent désormais un `Diamètre de montage (m)` réglable par pas de 5 cm, de 0,05 à 1,20 m. Le facteur est stocké par pièce (`PlacedPart::size_scale`) avec une valeur par défaut de 1 : les sauvegardes existantes se chargent sans migration et les pièces déjà placées gardent leur taille. Seule la section change, la longueur et la masse restent celles de la pièce ; un ordinateur de 0,30 m peut ainsi s'aligner sur un corps de 0,40 m sans adaptateur ni saut de diamètre.
- **La coiffe devient une pièce plaçable.** La coiffe ogivale ajoutée d'office au sommet du mesh a été supprimée. La palette propose `COIFFE` dans Structure et charge : longueur (0,10–3,00 m), diamètre de base (0,05–1,20 m) et profil conique, ogival ou parabolique. Sa masse dépend de l'enveloppe et du profil, son mesh est construit à partir de ses propres dimensions et le sommet de pile est fermé tant qu'elle est présente. La recherche `part:nose` est disponible dès le départ.
- **Persistance et compatibilité.** Le schéma véhicule reste 7. `size_scale` est absent des anciens JSON et vaut 1 à la lecture ; une valeur non finie ou hors bornes est refusée. La coiffe se sérialise comme les autres variantes de `PartSpec`, sans version de schéma supplémentaire.
- **Vérifications.** Nouveaux tests sur l'alignement de diamètre, le chargement d'une sauvegarde sans `size_scale`, les dimensions et le caractère terminal de la coiffe, le réglage joueur (longueur, diamètre, profil), le blocage d'ajout au-dessus de la coiffe, la suppression sans cap généré et la persistance du diamètre. `cargo fmt --all -- --check`, `cargo check`, clippy et les 346 tests du workspace restent verts. Vérification graphique à 1600 × 960 et 1280 × 720, au redimensionnement à chaud : coiffe ogivale visible et paramétrable, pièce retirée sans cap automatique, ordinateur et radar ramenés au diamètre du module, et plus d'avertissement de saut de diamètre pour ces deux pièces.

## 0.40.0 — Inspecteur de transformation avancé

- **Saisie numérique directe.** La pièce sélectionnée expose sa position locale (X, Y, Z en mètres) et sa rotation locale (X, Y, Z en degrés). Un clic sur une valeur ouvre la saisie : Entrée valide, Échap annule, les décimales et les nombres négatifs sont acceptés. Une valeur vide, non numérique, NaN, infinie ou hors limites est refusée avant toute modification du véhicule.
- **Précision indépendante.** Les boutons − et + suivent quatre pas : grossier (0,50 m / 15°), normal (0,05 m / 5°), fin (0,01 m / 1°) et très fin (0,001 m / 0,1°). Le choix est un état d'interface et ne touche pas au schéma de sauvegarde.
- **Remises à zéro.** Un bouton par axe, un pour les trois translations et un pour les trois rotations ; la transformation initiale se restaure en une seule action. La pièce racine reste verrouillée.
- **Une action, une annulation.** Chaque validation, pas ou remise à zéro appelle une seule opération de `MissileSession`; un glissement de gizmo n'enregistre qu'une opération au relâchement. Les valeurs de l'inspecteur suivent le gizmo et une valeur validée déplace le modèle et le gizmo.
- Le panneau défile dans la colonne de droite et les six lignes restent accessibles à 1280 × 720 comme à 1600 × 960 ; aucune version de schéma n'est modifiée.
- Vérification en jeu à 1600 × 960 et 1280 × 720, avec redimensionnement à chaud : saisie de valeurs négatives et décimales, refus d'une valeur hors limites et d'une entrée vide, annulation par Échap, pas fin, remises à zéro, restauration initiale, racine verrouillée, glissement du gizmo, Ctrl+Z/Ctrl+Y et persistance après redémarrage. `cargo fmt`, clippy et 341 tests restent verts.

## 0.39.0 — Contrats à profils, charges explosives et discrétion

- **Écran principal et six campagnes.** Le jeu démarre sur un écran titre qui liste `saves/slot_1` à `saves/slot_6` avec jour, crédits, technologies, contrats actifs et livraisons. On peut créer une campagne, charger une campagne existante, reprendre la campagne active et supprimer un slot après confirmation. Les fichiers de jeu ne sont écrits qu'après le choix d'un slot ; l'ancienne disposition 0.21 reste importée dans le slot 1. Le bouton `MENU` de la barre permet de revenir à cet écran. Vérifié en jeu à 1600 × 960 et 1280 × 768 : chargement du slot test, création, reprise, confirmation de suppression et suppression réelle.
- **Neuf profils de contrat.** Chaque famille de mission peut proposer plusieurs offres : frappe profonde, raid furtif, frappe de précision, supériorité aérienne, interception longue portée, défense ponctuelle, défense de zone, frappe de théâtre et rase-mottes naval. Chaque profil apporte son client, son brief, sa récompense, son échéance et une pondération différente du dossier design/programme. Le tableau de bord affiche au plus deux profils par famille, soit huit offres au lieu d'une par famille.
- **Charges explosives à trois grades.** Une pièce de charge explosive porte une masse réglable, une masse au lancement et un rendement abstrait. Le grade standard est disponible au départ ; les grades amélioré et avancé sont deux technologies de Matériaux (3 000 cr / 2 j et 5 000 cr / 3 j) et augmentent le rendement par kilogramme. Les contrats demandent une masse minimale et un rendement minimal.
- **Discrétion et apogée.** `FlightReport` mesure l'altitude maximale et en dérive une discrétion `100 × exp(-altitude / 7 000 m)`. Les profils bas exigent une discrétion minimale ; les profils hauts exigent une apogée minimale. Une preuve d'essai est refusée par un contrat dont la famille de vol diffère (`Ballistic`, `Cruise`, `ShortRange`). La séparation d'étage n'est exigée que pour la famille balistique : une conception courte portée ou de croisière à un seul étage reste éligible.
- **Un appareil, un contrat.** La livraison refuse désormais un design déjà livré à un autre contrat : l'empreinte structurelle ignore le nom, les ids de pièces et l'ordre des enfants, mais conserve l'arbre et les transformations d'attache. Renommer le véhicule ou toutes ses pièces ne crée donc pas un second appareil. Les preuves d'un essai ne sont enregistrées que sur les contrats compatibles avec son profil de vol.
- **Bureau des contrats lisible.** Les offres et les contrats actifs ont leur propre zone défilante ; le dossier reste fixe dans la colonne de droite. À 1280 × 768, les huit offres se parcourent à la molette sans emporter le dossier, et le rappel `MOLETTE : PARCOURIR` reste visible. La génération et le réapprovisionnement partagent la même limite de deux profils par famille.
- **Adaptateurs axiaux.** Un adaptateur relie deux corps de diamètres différents : diamètre d'entrée, diamètre de sortie, longueur et profil conique, ogival ou à épaulement. Il absorbe le saut de diamètre qu'il couvre, sa masse suit le profil et son mesh est un seul corps arrondi continu ; le nez prend le diamètre de sortie si l'adaptateur est en haut de la pile. Le schéma véhicule passe à 7 ; les schémas 3 à 6 et les anciennes listes d'étages se chargent sans changement.
- **Relecture de la session.** Le recalage de caméra au redimensionnement passe par `reframe_on_resize` (même biais, aucun décalage manuel perdu), le point des anneaux de gizmo est calculé par une seule fonction pour le dessin et le test de saisie, et les proportions du nez sont nommées. Aucun réglage visuel ni gameplay n'a changé.
- Maquette 3D retravaillée : sections rectangulaires arrondies au lieu de cylindres, joints de panneaux, tuyères en cloche avec disque de sortie, nacelles de turboréacteur avec entrée sombre et cône de nez, capteurs à sonde, plaques avioniques avec bandeau d'état, ailes continues (forme en plan et flèche conservées) et nez ogival plus court (2,2 rayons).
- Palette de matériaux militaire cohérente (olive, sable, gunmetal, graphite) avec métal sur les moteurs et accents émissifs discrets sur l'avionique ; aucun asset externe ni texture image. L'entrée du turboréacteur regarde le nez et les moteurs solides finissent sur une jupe de tuyère sombre.
- Les véhicules à réaction (missiles de croisière) s'éditent à l'horizontale, nez vers +X : sélection, points d'attache, glisser-déposer, gizmos, marqueurs aéro et caméra utilisent la même transformation. Les ailes du modèle « ailes » passent en montage opposé.
- Cadrage de l'atelier adapté aux deux largeurs : le modèle est placé dans la bande libre au-dessus du panneau central, avec un biais recalculé à l'ouverture et pendant un redimensionnement à chaud. Les poignées de gizmo passent de 1,5 m à 0,7 m et les anneaux à 0,5 m pour ne plus écraser un missile de 2,6 m et rester sous la barre supérieure en 1280.
- Vérification en jeu à 1600 × 960 et 1280 × 768 : ouverture directe, redimensionnement à chaud, picking d'une aile, translation par gizmo et anneaux de rotation ; `cargo fmt`, clippy et tests restent verts.
- Vérification du bureau des contrats à 1600 × 960 et 1280 × 768 : huit offres, molette sur la seule colonne des offres, dossier et contrats actifs accessibles, profils incompatibles signalés, charge et discrétion affichées dans les cartes et le dossier ; `cargo fmt`, clippy et 334 tests restent verts.
- Vérification de l'adaptateur : picking par cylindre au diamètre maximal, alignement des deux nœuds axiaux, absence d'avertissement de saut de diamètre, changement de profil et de diamètre de sortie, et même biais de cadrage après redimensionnement ; fmt, clippy et suite complète verts.

## 0.38.0 — Ergonomie du gizmo, angle d'aile et interface lisible

- Gizmo de transformation affiné : poignées et anneaux plus faciles à saisir, aperçu pendant le glissement et une seule entrée d'annulation par geste ; l'aide de l'atelier rappelle les commandes W/R.
- Angle d'aile réglable, avec un effet de jeu abstrait, annulable et visible dans l'inspecteur.
- Formes d'ailes distinctes dessinées dans l'atelier (droite, flèche, delta, trapézoïdale, géométrie variable) ; les segments restent alignés quand l'aile pivote.
- Police DejaVu Sans Mono embarquée dans `assets/fonts` : les accents français (É, È, à, é, ç…) ne s'affichent plus comme des carrés vides. La licence de la police est fournie avec le fichier.
- Traduction française complétée : atelier, inspecteur, bilan des étages, validation, programme (blocs, tâches, bibliothèque, débogueur), essais et contrats.
- Mise en page réparée : les boutons de blocs restent dans leur colonne, le résumé du véhicule ne déborde plus et l'aide raccourcie de l'atelier laisse le panneau VERSIONS entièrement visible.
- Vérification graphique en jeu à 1600 × 960 ; la suite de tests, `cargo fmt` et `cargo clippy -D warnings` restent verts.

## 0.37.0 — Atelier de construction et interface française

- Palette de construction filtrable par catégorie : tout, propulsion, structure, aérodynamique, capteurs et avionique. Le filtre est un état d'interface et ne modifie pas le véhicule.
- Arbre d'assemblage plus lisible : noms de pièces compréhensibles, points d'attache et étage affichés au lieu des identifiants internes.
- Instructions de construction clarifiées ; commandes de déplacement de sous-ensemble, symétrie, annulation et rétablissement rappelées dans l'atelier.
- Caméra de construction : F centre la pièce sélectionnée ; Début (Home) cadre automatiquement l'ensemble du véhicule.
- Points d'attache libres visibles sur la pièce sélectionnée, même hors glisser-déposer.
- Outil de transformation 3D inspiré de KSP : poignées colorées X/Y/Z pour déplacer une pièce, anneaux pour la faire tourner sur les trois axes, aperçu pendant le glissement et une seule entrée d'annulation par geste.
- Les transformations complètes (translation et rotation sur les trois axes) sont sauvegardées dans le schéma véhicule 6 ; les anciens véhicules chargent avec des transformations nulles.
- Rotation rapide des surfaces aérodynamiques avec Q/E par incréments de 15°, angle affiché dans l'inspecteur et opération annulable.
- Interface utilisateur traduite en français : navigation, atelier, contrats, recherche, programmation, essais, boutons, états et principaux messages.
- Ajout de `README.fr.md`. Les identifiants, fichiers et schémas de sauvegarde restent inchangés.
- Vérification graphique recommandée à 1600 × 960 et sur une fenêtre plus étroite.

## 0.36.0 — Programmation visuelle
- **The block language grows to 15 blocks in eight categories.** `omega-rules::program` now owns `BlockCategory` and assigns every block to inputs, conditions, logic, states, timers, outputs, modes or diagnostics; the editor renders one library section per category. Seven blocks are new: wait speed, wait pressure, wait flag, toggle flag, set flag, set mode / wait mode and mark. The existing seven and their hardware rules are unchanged.
- **A small, real state machine.** Four flags and one mode register are shared by all threads; `wait flag` and `wait mode` are gates that hold their thread until met, then record an anomaly and continue at the timeout. `mark` only writes an event. Nothing carries guidance or targeting logic.
- **Connections are explicit and checked.** Threads are chains drawn with a connector between consecutive blocks; the selected block is the insertion point (library click splices, `DUPLICATE` copies, `MOVE` reorders). The rules refuse a block after `Lock target`, an ignition of a stage that was already separated in the chain, and a second `Lock target`.
- **Structured validation.** `program_issues()` returns one issue per problem with severity, thread and block; `validate_program()` remains the error-only string view the flight uses. Warnings are separate: empty thread, condition nobody sets, 0 s wait, thin computer memory. The panel shows error and warning chips plus the first six rows.
- **Reusable library.** `ProgramLibrary` holds up to 12 named programs in `programs/library.json`; `SAVE COPY` stores the working program, any entry can be loaded on another vehicle (validation then names the missing hardware) or deleted. The working program keeps its own file.
- **Seeded debugger.** `RUN TRACE` flies the current program with `FlightConfig::trace`, records every executed block (time, thread, index, label, timeout) in `FlightReport::trace`, and the panel shows the trace, timeout count, flight summary and anomalies. Any edit clears the run; batch trials do not pay for the trace.
- Full description and boundaries: `docs/PROGRAMMING.md`. Tests: 304 (88 application, 18 `omega-data`, 127 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean. Not verified on screen: the new palette sections, connector lines, library and debugger panels were not rendered (no display on this machine), so the 1600 x 960 layout still needs a graphical check.

## Unreleased — Onboard electronics (0.35.0)
- **A family with one shape.** `omega-rules::avionics` defines seven categories (computers, control, sensors, navigation, communication, power, diagnostics), five mission modules and one `AvionicsSpec` describing mass, abstract cost, power draw or supply, complexity, reliability and the effects it grants. The legacy computers and sensors share the same view through `spec_of()`, so every electronic part is summarised once, by `Vehicle::avionics()`.
- **Five new parts.** Control augment (+15 % authority, needs a standard computer), inertial platform (+6 lock points), datalink (+4 lock points, needs navigation), power cell (60 W of mission supply, stackable) and self-test suite (halves the seeded fault exposure). The palette's AVIONICS section grows to eight entries; the 3D view gives each module its own disc colour.
- **A real power budget.** Drawing modules need a cell: no supply is an error (`the mission electronics draw N W but no power source is fitted`), a deficit names both figures, and a margin below 25 % is a warning. The mission bus is drawn by the new modules only, so a design without them never needs a cell.
- **Compatibility is a rule.** The rules refuse a duplicated non-power module, a datalink without navigation and a control augment on a basic computer, and warn about radial mounting, thin headroom and modules that would be dropped before the last stage. `Vehicle::issues()` merges them with the assembly and aerodynamic rules, so the validation panel lists them unchanged.
- **Effects reach the trial.** The flight reads the figures once: navigation bonus on the lock roll, authority multiplier on the control term, fault multiplier on the seeded surface fault. `FlightReport` carries `avionics` and `surface_fault`, and `omega --trial` prints both. Without modules every multiplier is 1 and every seeded roll is unchanged.
- **Research and UI.** Two technologies unlock the family: *Flight control electronics* (computing, 6 500 cr, 9 days) and *Navigation and datalink* (sensors, 7 500 cr, 10 days); the tree is now 20 technologies for 77 000 cr and 86 days, and the budget test pins it. The inspector quotes a module's cost, draw, complexity, effect and dependency, and the stage panel summarises devices, power margin and cost.
- **`designs/vehicle.json` moves to schema 5.** Schema 3-4 graphs and the migrated stage lists load unchanged and rewrite as 5; a newer schema is refused. Design hashes cover the modules, so old evidence goes stale through the normal check.
- Full description, values and boundaries: `docs/AVIONICS.md`. Tests: 291 (83 application, 18 `omega-data`, 119 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean. Not verified on screen: the new palette section, inspector lines, stage summary and 3D colours were not rendered (no display on this machine), so the 1600 x 960 layout and the module discs still need a graphical check.

## Unreleased — Workshop completeness (0.34.0)
- **The palette is grouped by category.** `omega-rules::vehicle` now owns `PartCategory` (propulsion, airframe, aerodynamic surfaces, sensors, avionics) and `PartSpec::category()`; the workshop builds one section per category from `PartTemplate::ALL` instead of a hard-coded row list, so a new part family lands in the right place. A test pins that every category has a part and every palette label is unique.
- **The assembly is shown as a tree.** `Vehicle::tree_entries()` walks the attachment tree depth-first with the indent, parent, node and derived stage of every part; the workshop renders it as clickable rows. Tested with a migrated stack and with three radial parts attached out of order (node order is stable).
- **Compatibility has one structured source.** `Vehicle::issues()` returns errors and warnings with a severity and the part to select; `validate()` and `warning_details()` are now views over it, so every message is unchanged while the board stops pairing strings with parts by hand. The workshop's new validation panel lists them, counts errors and warnings and turns each row into a select shortcut. Tests pin the severity, the part id and the unchanged error list.
- **Detailed inspection.** The selected-part panel adds the attachment (parent and node), stage, dimensions, part category and any research the current build still needs, next to the existing parameters, design choices and dry/propellant/thrust figures.
- **Contract check in the workshop.** New Bevy-free `omega-rules::compliance::contract_preview` maps a contract's own requirements to the design figures the vehicle determines (launch mass, stability, lift, control authority) and marks each `MET`, `CLOSE` (within 5 %), `MISSED`, or `TRIAL` when only a flight can prove it. The workshop panel shows the selected active contract, its due day and those rows; no second flight model and no evidence is created. Tests cover the requirement order, mass verdicts and aero verdicts.
- **Vehicle summary.** The stage panel now quotes length, part count and stage count next to total mass.
- Version, window title, README and roadmap baseline at 0.34.0.
- Tests: 278 (82 application, 18 `omega-data`, 107 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean. Not verified on screen: the new panels were not rendered (no display on this machine), so the 1600 x 960 layout and the tree/validation scrolling still need a graphical check; the rules behind them are covered by tests.

## Unreleased — Post-aerodynamics follow-ups (0.33.0)
- **The airframe choices now pay.** Reliability and complexity had no gameplay reader: a fragile airframe (below 60 reliability) now draws at most one seeded surface fault per trial, halving control authority and recording an anomaly, while a sound design never does. The light build is therefore a real trade instead of a free mass saving.
- **Dossiers pay for quality.** Delivery used to credit the offer's fixed fee whatever the score; the fee is now nominal at the 60 % acceptance threshold and rises to +15 % for a perfect dossier, so the added aero requirements and the batch/AB tooling have an economic payoff.
- **Each family asks its own trait.** Cruise offers add a soft `LIFT ≥ 40`, air-to-air offers a soft `AUTHORITY ≥ 25`, and the existing surface-to-air `STABILITY ≥ 20` stays. Lift and control authority join `MissileTrial` and its evidence, all bound to the design hash.
- **Research pacing measured and retuned.** The tree now holds 18 technologies for 63 000 cr and 90 serial days, which made the aerodynamics branch a wall; durations drop to 67 days in total (aero department 16 days) without touching a cost, and a test pins the budget.
- **Effect tables are data.** The twenty design-choice effects moved to `crates/omega-rules/src/aero_effects.json`, embedded at compile time and validated against every enum variant; `omega --aero-report` renders them as `docs/AERO_BALANCE.md`.
- **Explainability and a design doctor.** `aero::breakdown` reports each surface's contribution and the panel quotes the selected one; warnings now carry the part they concern and are rendered as buttons that select it, with a plain-text accessor kept for callers that only print.
- **A screenless harness.** `omega --trial <slot-dir> [--seed N] [--air]` flies the saved design and program without a window and prints JSON; `--revisions` replays the working copy and the ten newest recorded revisions with one seed and prints the deltas, which is how this lot's flight behaviour was verified without a display.
- **Designs move between saves.** `omega --export-design` and `--import-design` write and read the schema-4 vehicle, refusing a design that does not validate.
- **Iteration shortcuts.** A stale contract now offers `RE-FLY`, which records fresh evidence from the current design in one press, and the workshop gets `FINS` and `WINGS` presets that show what a stabilised rocket and a lifting cruise vehicle look like.
- Version and window title aligned at 0.33.0; the research overview in the README and roadmap now counts 18 technologies in six departments.
- Tests: 270 (81 application, 18 `omega-data`, 100 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean. Not verified: the workshop panel and contract card were not re-checked on screen after these changes; the flight, reward, contract and export paths were checked headlessly.

## Unreleased — Aerodynamics: wings, fins and control surfaces (0.32.0)
- **The airframe is now a design decision.** Three leaf parts mount on radial slots: `Wing` (lift for cruise), `FinSet` (passive stability) and `ControlSurface` (authority when moving, stability when fixed). Each carries span, chord and, for wings, sweep, plus discrete choices: planform (straight, swept, delta, trapezoidal, variable geometry), profile (symmetric, classic, low drag, high lift, experimental), stabiliser role (rear, front, blended, combined), fin layout (symmetric/asymmetric), control family (classic, integrated, combined, fixed) and a standard/light build. All are mirrored with `X` like any other radial part.
- **One Bevy-free rule module.** `omega-rules::aero` aggregates the tree once into `AeroFigures`: reference, wing, fin and control areas, area-weighted sweep, stability, control authority, lift, drag index, complexity, reliability, aerodynamic mass, surface drag area, CG, CA, margin, band and balance. Every coefficient is one `Effect` table per family in that module; scores normalise against 0.30 m² of surface (0.90 m² for lift, 0.60 m² for drag). The CG is mass-weighted over all parts, the CA is area-weighted over the surfaces only, and the acceptable band puts the CA 2–10 % of the length behind the CG.
- **Flight uses the figures without a new simulation.** Surface drag area joins the body term, lift lowers the air-breathing minimum speed (90 m/s by up to 45 %, floor 55 %) and the pre-pitch-over gravity loss (up to 35 %), stability damps the seeded drag spread (up to 60 %) and authority/stability raise the altitude-hold rate (up to 50 %). `FlightReport::aero` quotes the summary. A design without surfaces leaves every term at its previous value; seeds stay deterministic.
- **Compatibility and warnings.** Surfaces mount on radial nodes only and cannot carry parts; integrated control surfaces need a standard computer and combined ones an advanced computer. Oversized spans, aerodynamic mass above 20 % of launch mass, a control surface without a fixed surface, an asymmetric fin set and a CA ahead of the CG are warnings, not refusals.
- **Workshop and 3D.** Aero palette row, plate meshes (blue wings, green fins, grey fixed and amber moving surfaces), oriented-box picking, span/chord/sweep with the wheel and `+`/`-`, design-choice buttons, yellow CG and cyan CA markers with their link and acceptable band, and an aero panel with an A/B snapshot.
- **Boards stay usable with the airframe.** The five aero technologies get their own AERODYNAMICS department (so no column overflows the research board, which now scrolls), and the workshop stage list is capped and scrollable so the selected-part inspector (figures, choices, remove) stays on screen on a fully equipped design. Palette, drag, wheel and keyboard edits now save immediately instead of waiting for the next inspector click, and recording or clearing the A/B snapshot redraws the aero panel.
- **Research and contracts.** `Aerodynamics` (`part:wing`), `Fin stabilisation` (`part:fin`), `Control surfaces` (`part:control_surface`), `Advanced profiles` (`aero:advanced_profiles`) and `Light aero structures` (`aero:light_structures`) form the new **AERODYNAMICS** department with prerequisites. Surface-to-air offers add a soft `STABILITY ≥ 20` requirement; its evidence is bound to the design hash like every other proof.
- **`designs/vehicle.json` moves to schema 4.** Schema 3 graphs and the migrated stage lists load with no surfaces and rewrite as 4; a newer schema is refused. Any surface edit changes the design hash, so old trial evidence and dossiers become stale.
- Tests: 257 (74 application, 18 `omega-data`, 94 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean. Flight tests pin the no-surface invariants, fin damping, wing lift at 70 m/s and the control-surface settling time; a headless sweep of every planform, profile, role, layout and control family at extreme spans and chords pins every score to 0..100 and keeps the CG/CA pair inside the vehicle, and a contract test checks that the soft stability requirement scores without ever rejecting a dossier.

## Unreleased — Workshop drag interaction (0.31.1, second step)
- **Parts are built by dragging in 3D.** A camera ray picks the part under the pointer (`src/picking.rs`), the palette entries carry their template, and a drag from the palette to any free node attaches there; the nodes glow while dragging, the nearest one within 28 px turns green and the hovered node gets a ghost cube. Dragging an existing part moves its whole subtree, refused for the root and for cycles. A 5 px threshold keeps a plain click a pure selection, and `Delete` / `Backspace` removes the selection (whole subtree, with a count).
- **Radial symmetry is linked.** `X` cycles 1 → 2 → 3 → 4; a radial drop creates the copies on consecutive free slots with one shared `symmetry` group id, and an occupied slot cancels the whole drop without leaving copies behind.
- **Undo/redo covers every edit.** Up to 50 `Vehicle` snapshots in `MissileSession`, reached from the `UNDO` / `REDO` buttons or layout-independent logical `Ctrl+Z` / `Ctrl+Y` (on AZERTY the physical key is `W`, so the physical key code would never match). Presets clear the history.
- **Direct adjustment.** The wheel over the selected part changes its main parameter (length, burn time, engine scale or module mass) and suppresses the camera zoom for that frame; the `SYMMETRY`, `UNDO` and `REDO` buttons sit under the palette.
- **A wheel gesture is one undo entry.** Notches on the same part and value less than 500 ms apart share the snapshot taken by the first notch, so a continuous adjustment no longer floods the 50-entry history; a button edit, an undo/redo, an add/move/delete or a 500 ms pause starts a new entry.
- **Escape cancels a drag first.** While a drag is active, Escape clears it and keeps the workshop open (`Drag cancelled`); only a second Escape returns to the island. Previously Escape left the board before the workshop could see the key.
- Verified in game at 1600×960: palette→top and palette→radial drops, a subtree move, delete + undo, redo, `X` with three linked copies (`Added 3 copies to stage 2`), click-select that no longer relocates the part, a single `Ctrl+Z` restoring a three-notch wheel adjustment (1.50 → 1.80 → 1.50 m), `X` ×4 with `Added 4 copies to stage 2`, and Escape cancelling a palette drag with no part added.
- Tests: 228 (68 application, 18 `omega-data`, 71 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean.

## Unreleased — Workshop assembly graph (0.31.1, first step)
- **The missile is now a tree of parts.** `omega-rules::assembly` stores `PlacedPart`s attached to `top`, `bottom` or one of four radial nodes, refuses cycles, occupied nodes, unknown parents, multiple roots, an engine without a tank in its stage and a computer outside the last stage, and reports a diameter step as a warning. Stages are derived: a decoupler (or an explicit separation edge) starts a new one.
- **`designs/vehicle.json` moves to schema 3.** Old stage-list files are migrated on read (chained bottom to top, separation edges at former stage boundaries) and keep their stage figures; a newer schema is refused with a message. Existing hash-bound evidence therefore goes stale, as expected and by design.
- The workshop shows the derived stages and part tree, selects parts by id, adds parts above the selected part (or the highest one), replaces the flight computer in place and reuses the same 3D layout with radial offsets. The stage list no longer has "+ STAGE" buttons.
- Not yet in this step: drag-and-drop, node picking, linked symmetry placement, undo/redo, direct dragging of parameters, live flight forecast and the contract requirement bar. `docs/DECISIONS.md` records the choices and the open interaction values.
- Tests: 217 (57 application, 18 `omega-data`, 71 `omega-rules`, 71 `omega-sim-core`); fmt and clippy clean.

## Unreleased — Project consistency and maintenance baseline (0.30.0)
- Align the package version, lockfile, window title, README and current roadmap at 0.30.0.
- Replace outdated architecture and vision descriptions that still referred to the removed civil loop, legacy editor and pre-missile workflow.
- Record the current repository baseline: missile-only gameplay loop, versioned save slot, research-gated contracts and the existing CI checks. No gameplay or save-schema change is intended by this maintenance pass.

## Unreleased — Research-gated contracts and the end of the inherited loop (lots 4-5)
- **Research now constrains the contract office.** Each offer names the `family:*` capability it needs: air-to-surface and surface-to-surface work uses the starting ballistic family, surface-to-air and air-to-air need their own family, a cruiser needs the cruise family, and difficulty 4-5 surface-to-surface offers need the anti-ship family. An unresearched family produces no offer; the empty board lists the missing technologies by name.
- **Active contracts follow `ExtraActiveProjects`.** The office starts with one slot and `Parallel project management` raises it to two. Each contract keeps its own evidence, dossier, deadline and selection, and a trial feeds every active contract. Accepting an offer for a locked family (only possible from an older save) or beyond the slot count is refused with a readable message.
- **The civil loop is gone.** Deleted the assembly bench, polygon sketch, instrument profile, chamber sequencer, thermal/truss/mesh studies, job workers and civil contracts, with their boards, keyboard handling and tests. The app no longer depends on `omega-sim-core`, which stays as a standalone library. The revision log is missile-only (schema 3) and `SaveFile` only knows the five missile files.
- Saves: campaign schema 2 still reads schema 1 credits/research/day; contract schema 1 migrates its single active contract into the new list; assembly-era revision logs open empty instead of failing.
- Verified in game on a fresh campaign: two ballistic offers with the four locked families named, a second acceptance refused at 1/1, the second slot opened by research, two active contracts listed side by side, and a surface-to-air offer appearing after its family was researched. Workshop, program and trial boards still render and record evidence.
- Tests: application 56, `omega-data` 10, `omega-rules` 66, `omega-sim-core` 71; `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Unreleased — Missile program editor (lot 1 of `docs/ROADMAP_OMEGA.md`)
- **New `program_editor.rs` and PROGRAM tab (`F4`):** the onboard program is edited as parallel threads of the seven blocks of `omega-rules::program`. The palette is driven by the hardware actually fitted to the vehicle, so `hold speed` needs a pitot tube, `hold pressure` a barometer, `lock target` a radar and `separate stage` a second stage.
- Live thread and memory meters against the flight computer class; the editor refuses a block when the computer memory or the parallel-thread limit is reached, and names the missing hardware otherwise.
- Blocks can be appended to the selected thread, selected, tuned with `-` / `+` (wait, stage number, throttle, speed, pressure), moved up or down and removed. Validation runs on every edit and reports the thread and block at fault.
- The program is persisted to `programs/missile.json` through the save slot on every change, reloaded at startup, and carries a content hash ready to bind trial evidence.
- The 0.6 chamber sequencer remains available as the **SEQUENCER** tab, now explicitly labelled legacy in its header.
- Also merged: abstract military contract rules from `feature/0.29.0-military-contracts` (`omega-rules::contracts`: mission and vehicle families, requirements, offer generation, phases, submission evaluation). Its capability test depended on a seeded difficulty roll; it now pins the capability instead.
- Verified on screen: palette gating, thread/memory meters, validation, tuning, reordering, removal, clear, persistence and reload. Two defects found and fixed: a program longer than ~20 blocks pushed the selected-block controls off the window bottom, and the validation list stopped at six issues without saying so.
- The block list now caps its height and scrolls with the wheel (the offset survives the rebuild after each edit), the selected-block panel keeps its place, and the wheel no longer zooms the island camera behind the board.
- Tests: application 130, `omega-data` 10, `omega-rules` 56, `omega-sim-core` 71; all pass. The editor is covered by 9 new unit tests.

## Unreleased — Project renamed to Omega
- Renamed the Cargo package and binary from `aerolab` to `omega`, and the workspace crates from `aerolab-sim-core`, `aerolab-data` and `aerolab-rules` to `omega-sim-core`, `omega-data` and `omega-rules` (the last two directories moved to `crates/omega-data` and `crates/omega-rules`).
- Renamed the worker-thread environment variable `AEROLAB_WORKER_THREADS` to `OMEGA_WORKER_THREADS`.
- Updated the window title, HUD banners, HTML report headers, thread names and documentation to Omega. The public repository is `Sts-simon/Omega`; entries below keep the former name.
- No save-schema or behavioural change: the slot layout, `designs/vehicle.json` and existing campaigns load unchanged.

## 0.18.0 — Mouse-first navigation and contextual UI
- Added a persistent top navigation bar for Island, Contracts, Workshop and Laboratory views.
- Added scrollable contextual tool shelves for the Contract Office, Workshop and Computation Lab.
- Added mouse-clickable component selection, axis translation, axis rotation and per-axis dimension controls in the Workshop.
- Added CAD-style camera controls: right-drag orbit, middle-drag pan and wheel zoom. Pixel-based touchpad scrolling is normalized separately from line-based wheel input.
- Prevented camera movement and world-space picking while the pointer is over an interactive UI panel.
- Repositioned and made HUD panels scrollable so navigation, inspectors and result queues do not cover one another.
- Kept the scientific solvers and saved assembly/campaign/polygon schemas unchanged.

## 0.17.2 — Sketch system visibility and application compile cleanup
- Matched the crate-level Bevy schedule visibility with `pub(crate)` sketch resources, marker components and systems.
- Kept the polygon sketch session as the exported resource API while restricting internal render/query types to the application crate.
- Used the assembly mesh work-plane height in the research job summary.
- Removed an unused contact-tolerance constant and retained the point-add convenience method in the normal drawing path.
- Exposed undo availability in the workshop HUD and removed the resulting dead-code warnings.
- No save schema changes; all 0.17.1 sketch and assembly files remain compatible.

## 0.17.1 — Workspace integration compile fixes
- Fixed region-scoped mutable ring lookup in selected-vertex deletion.
- Fixed optional coordinate extraction in `vertex_at` with `Option::copied`.
- Avoided overlapping immutable/mutable borrows of the polygon sketch `ResMut` when entering hole mode.
- Split the 21-system Update chain into three nested chained groups, preserving execution order while respecting Bevy tuple arity.
- Removed unused re-exports in `design` and `jobs`.
- No data schema changes.

## 0.17.0 — Polygon vertex editing and snapping
- Added an explicit vertex-edit mode with visible handles for outer contours and holes.
- Added click-and-drag movement with live updates to only the incident edges and selected marker.
- Added 5 cm grid snap, nearest-vertex capture within 12 cm, a snap toggle and Shift bypass.
- Validated edited regions before commit; invalid geometry or a failed save restores the original vertex.
- Added safe selected-vertex deletion with a three-vertex minimum per ring; deleting an interface vertex shared by multiple regions is blocked.
- Moving a welded vertex moves every coincident region copy together to preserve shared material interfaces.
- Kept polygon sketch schema 1 unchanged for compatibility with 0.16.0 files.
- Added tests for snapping, hole vertex identification, drag cancellation, invalid-move rollback, self-snap exclusion and shared-interface vertex behavior.

## 0.16.0 — Interactive polygon sketch and multi-region meshing

- Added an interactive Workshop polygon sketch mode with mouse-added vertices, contour validation, hole drawing, multiple regions, material selection and thickness controls.
- Added persistent sketch documents at `saves/polygon-sketch.json` and in-scene colored contour visualization.
- Added a multi-region polygon mesh adapter that welds complete shared edges and preserves element material/thickness/source provenance.
- Added overlap and contact validation, plus Computation Lab `P` to queue a sketch-derived thermal study.
- Added tests for material interfaces, node welding, overlap rejection, point-only contact rejection, input limits and JSON round-trip.

## 0.15.0 — Concave polygon meshing and holes
- Added `PlanarPolygonRegion` and `mesh_planar_polygon` to the independent simulation crate.
- Added constrained triangulation using `earcutr` for simple concave outer contours with up to 64 holes.
- Added contour cleanup, winding normalization, simple-ring checks, strict hole containment/non-contact checks and total vertex budgets.
- Added triangulated-area verification and full triangle validation, preserving material, thickness and source-part provenance.
- Added a headless polygon thermal study example and a JSON input deck containing an L-shaped region with two interior openings.
- Added tests for concave polygons, one and multiple holes, reversed winding, repeated closure points, collinear vertices, invalid holes, self-intersections and thermal-solver integration.
- The workshop editor remains rectangular; polygon editing and multi-region polygon interfaces are future work.

## 0.12.0 — Scientific visualization and report viewer
- Added a Bevy-independent HTML/SVG and CSV exporter for thermal and civil truss results.
- Added block-averaged thermal field heatmaps with full-resolution CSV data and energy diagnostics.
- Added structural reports showing undeformed/deformed geometry, tension/compression coloring, supports, applied loads, member tables and nodal/member CSV values.
- Added a civil truss study to the in-game research queue (`G`) and opening the latest HTML report in the default browser (`O`).
- Kept exact JSON solver reports and added result-shape, identifier and finite-value validation before export.
- The deformed shape is visually amplified and explicitly labeled; exports preserve unamplified SI quantities.

# Changelog

## 0.26.0 — Missile workshop and working research (0.25 completed)
- **Missile workshop** (`missile.rs`, `campus/board.rs`): the WORKSHOP tab now builds a staged vehicle in 3D from the parts of `aerolab-rules::vehicle`. Add parts to the active stage, add or remove stages (up to 4), select a part and resize it with +/- buttons (lengths, diameters, solid-motor burn time, engine scale, module mass), remove it, or start from a preset (two-stage rocket, cruise vehicle) or an empty vehicle. The single flight computer always sits in the last stage and follows new stages.
- Live figures per stage: mass, mass carried, thrust, burn time, thrust-to-weight ratio (red below 1 and amber below 1.3 for the first stage) and delta-v; total mass; computer class with its thread and memory limits; the programming blocks the installed sensors unlock; research still missing; design validation messages.
- The 3D stack is rebuilt on change, with a highlight on the selected part and a nose cone; the camera centres on the vehicle when the workshop opens. The vehicle is saved in the slot (`designs/vehicle.json`) on every change and reloaded at startup (invalid parts are rejected on load).
- **Research Centre is now playable:** 13 technologies in five departments with cost, duration, prerequisites and status (RESEARCHED, N DAYS LEFT, LOCKED, NEED x cr, LAB BUSY, START RESEARCH). One project at a time; the new **NEXT DAY** button advances the game clock (day counter in the navigation bar). Credits are deducted when research starts; research state and day are saved in the campaign (older campaigns load with the defaults).
- **Parts are gated by research:** locked parts are greyed and explain which technology to research (technology names, not internal ids). Capabilities come from the starting set plus completed research.
- The old generic assembly bench is kept as the **LEGACY BENCH** tab (and from the active-contract card), because civil contracts still run on it until the missile-family contracts arrive.
- Not done: programs are not edited in the application yet, there is no trial screen, contracts are still the civil ones and do not use the missile vehicle, and the missile design is not part of revisions or evidence. Research does not yet limit contract families or the number of active projects (those grants exist but nothing checks them).
- Tests (Linux sandbox): `sim-core` 71, `aerolab-data` 10, `aerolab-rules` 51, application 121; all pass. Exercised under a virtual display with software rendering: open the workshop, select and resize a part, click a locked part, start a research project, advance days until it completes, and add the newly unlocked radar.

## Unreleased (0.25 groundwork) — missile design core and research tree
- Game direction restated in `docs/VISION_MISSILES.md`: contract for a missile family, KSP-style staged build with resizable parts, sensors that unlock programming blocks, a computer that limits threads and memory, seeded trials, delivery of a plan (no production). Fictional and abstract by design; the lock block is a probability roll, with no guidance logic.
- `aerolab-rules::vehicle`: parts (solid motor with length, diameter and burn time; tanks; liquid and air-breathing engines with a scale; sensors; computers; decouplers; mission module), validation, per-stage mass, thrust, burn time, thrust-to-weight and delta-v, installed-hardware to block mapping, and missing-research check.
- `aerolab-rules::program`: seven blocks (wait, ignite, separate, throttle, hold speed, hold pressure, lock target) validated against the hardware and the computer (threads, memory), with lock target required to close its thread.
- `aerolab-rules::flight`: seeded, deterministic abstract flight test with events, anomalies (propellant dropped unburnt, out-of-order separation, lock above radar limit, radar jettisoned, cannot lift off, nothing ignited) and a report of speed, path, altitude, mass and thrust over time.
- Research tree rewritten for this loop (missile families, propulsion, sensors, computers, design tools) plus a set of starting capabilities. 51 tests in the rules crate.
- **Not wired into the application:** none of this is visible in the game yet.
- (Earlier in this entry) `aerolab-rules::research`: technologies with department, cost, duration, prerequisites and grants (named capabilities such as `part:cylinder`, `analysis:truss`, or +1 active project); tree validation (unique ids, known prerequisites, no cycles); one research project at a time; `start`, `advance_days`, capability and project-limit queries; JSON-serializable state. A 10-node starting tree covers the five departments. 6 unit tests (rules crate: 25/25).
- **Not wired into the game yet:** no game clock, no research board with START buttons, no persistence in the campaign, and no facility checks the capabilities. The Research Centre still shows PLANNED cards.

## 0.24.1 — Interface overhaul (cards, status chips, readable workshop)
- **Shared look** (`ui_kit.rs`): one palette, rounded cards with a coloured top edge, status chips, and buttons with tones (primary, secondary, danger, ghost, disabled) that react to hover and press. Thousands separators for credits.
- **Island:** a title with live stats and five facility cards (F1-F5 chips, short description, OPEN button) instead of a text wall.
- **Contract Office** (`campus/board.rs`): offers are cards (domain colour, difficulty pips, key limits, reward and deadline, ACCEPT; disabled look when two projects are active). Active projects show a status chip, a 4-step progress bar (design, analysis, field test, delivery) and context buttons (SELECT, WORKSHOP, COMPUTE, DELIVER). The selected project's dossier is a table of coloured chips (OK / THIN / FAIL / STALE / OPEN) with a verdict, next to the last completed contracts. Boards are rebuilt only when the data they show changes.
- **Workshop:** the left shelf now uses compact rows (35 stacked buttons became about 18 buttons in rows; rotation and size remain in the exact inspector); the polygon-sketch panel is hidden unless sketching; mouse/keyboard help is folded behind `H` / SHORTCUTS; a project card at the bottom shows the dossier chips, issues, profile and SUBMIT DESIGN. Panels no longer overlap. Record/restore revision are buttons.
- **Status messages** appear in the navigation bar (they used to be buried in the text panel and never expired; they now fade after 7 s). The bar also shows credits, active projects and completed contracts.
- **Research Centre:** five department cards marked PLANNED (the tree itself is still version 0.25).
- Not redone yet: Computation Centre and Programming Studio still use the earlier text panels; the instrument-profile editor (F11) is still a text panel; no scrolling inside boards (they are laid out for windows about 960 px tall).
- Tests: `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 19/19, application 111/111. The interface was exercised under a virtual display with software rendering: opening every view by clicking, accepting an offer, submitting a design from the project card, and reading the dossier.

## 0.24.0 — Design revisions, comparison and live budget (partial)
- **Revisions** (`design/revisions.rs`): `Ctrl+B` records the current assembly as a numbered revision whose parent is the previous head; an unchanged design is refused. `Ctrl+Backspace` restores the latest revision. The log is saved in the slot (`designs/revisions.json`, schema 1, validated on load) and reloaded at startup; it keeps at most 100 revisions (the oldest is dropped and its children detached).
- **Comparison** (`aerolab-rules::diff_parts`): the Workshop inspector shows what changed since the latest revision (parts added, removed, moved with the largest distance, resized, mass delta, change of CG offset in percentage points).
- **Live budget** in the Workshop inspector: mass (with its analysis uncertainty), envelope length and CG offset as a fraction of length.
- On-screen text fixes found with screenshots: characters missing from the default font (`x`, degrees, minus, plus-minus) are now ASCII; the Research Centre text no longer announces a past version; version labels read 0.24.0 (the programming studio still said 0.10.1).
- **Not done from the 0.24 plan:** typed vehicle modules, axial (vertical/horizontal) stacking by vehicle family, and a side-by-side view of two revisions. Revisions are linear snapshots of the whole assembly; there is no branch selection UI yet.
- **Known UI defects seen in screenshots (not fixed):** the Contract Office text is taller than the window with 5 offers, so the verification matrix is only readable in the Workshop; in the Workshop several panels overlap (polygon-sketch panel over the component inspector, project panel over the sketch panel, and its last lines fall below the window); the keys `1`-`5` also add components in the Workshop, so they do not accept contracts there.
- Test status (Linux sandbox, software rendering): `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 19/19, application 109/109. The game was also run under a virtual display to take the screenshots, which exercised: opening every view, accepting a contract, submitting a design, recording two revisions, comparing, restarting and reloading revisions and the campaign.

## 0.23.0 — Evidence, verification matrix and dossier-gated delivery
- Contract limits are now turned into requirements (`campus/verification.rs`): mass, unit cost, balance (when present), power, temperature range, storage, uptime and data completeness.
- Every check produces **evidence bound to the design hash** and stored in the campaign (`ActiveContract.evidence`):
  - at design submission, *analysis* evidence: mass, balance and cost derived from the real geometry with an analysis uncertainty (mass 3%, cost follows it), plus the exact values declared in the instrument profile;
  - at the field test, *test* evidence for uptime and data completeness, with the standard error of the 1,000-sample measurement (about 0.6 points at 96%) as uncertainty.
  Uptime and data completeness can only be proven by a field test, so they show as open until one is run.
- The selected contract shows a **verification matrix** in the Contract Office, Workshop and Computation Centre (`[OK]`, `[THIN]`, `[FAIL]`, `[STALE]`, `[OPEN]`) with value, uncertainty, margin and method, and a verdict line. Evidence becomes `[STALE]` as soon as the assembly changes.
- **Rule changes (gameplay):** (1) submission now also blocks a mass or cost whose analysis uncertainty crosses the limit (a "thin margin"); (2) delivery requires the generated dossier to be accepted: a field test that passes by less than one standard error is no longer enough; (3) completed contracts record the dossier score. Campaigns saved before 0.23 are upgraded transparently: evidence is rebuilt at delivery time from the current design and field-test report.
- The cost formula was extracted into `estimated_unit_cost_cr` (unchanged values).
- Test status (Linux sandbox): `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 15/15, application 104/104. Not run on screen; the new HUD text and the delivery gating have only been exercised through unit tests.
- Still open: `InstrumentProfile` is still a set of declared numbers; the matrix is text in the HUD, not a dedicated panel; programs and revisions are not part of the evidence yet (`program` is always empty).

## 0.22.0 — Shared data layer, verification rules and a geometry-derived requirement
- Added `crates/aerolab-data` (no Bevy): `ContentHash` change-detection fingerprints, crash-safe `write_atomic` (temp file, fsync, rename, one `.bak` generation) and `SaveSlot`, a versioned save slot whose `manifest.json` (per-file hash and size) is committed last. Includes a one-time, non-destructive import of the four 0.21 save files.
- Added `crates/aerolab-rules` (no Bevy): `Requirement`, `Estimate` (value plus uncertainty), `Evidence` bound to design/program hashes, `evaluate_requirement` (Verified / Marginal / Failed / Stale / Unverified / MethodNotAllowed), `build_dossier` (verification matrix, weighted score, acceptance decision with reasons) and `derive_metrics` (mass, length, centre of gravity and balance ratio from real part geometry).
- Contracts now carry an optional geometry-derived balance requirement (`max_cg_offset_ratio`, tighter at higher difficulty). `evaluate_design` checks it through the rules crate, so moving parts in the Workshop changes whether a design passes. Older campaign files load unchanged (the field defaults to none).
- **Save slot is now used by the application** (`src/save.rs`): campaign, assembly, polygon sketch and program are written to `saves/slot_1/` (`campaign.json`, `designs/assembly.json`, `designs/polygon-sketch.json`, `programs/lab_sequence.json`) through atomic writes, with a hashed `manifest.json` committed last. On first access, existing 0.21 files in `saves/` are imported once; they are left untouched and ignored afterwards. A file that no longer matches its manifest entry is reported instead of loaded (campaign: a new campaign starts, and the previous generation stays as `<name>.bak`).
- **Not yet wired:** the dossier, evidence store and verification matrix exist as library code but are not yet shown or stored by the campaign. Part rotations are ignored by the metrics. The save slot is fixed to `slot_1` (no slot selection UI).
- No change to the assembly, program or polygon-sketch schemas.
- **Build fixes needed for Bevy 0.20.1 / rustc 1.97:** the deprecated `Button` marker is no longer usable as a value, so buttons now spawn `Interaction` + `FocusPolicy::Block` and two `With<Button>` query filters were removed; nine component/resource types became `pub(crate)`. Compile-checked only, not run on screen.
- **Fixes found by running the tests:** polygon-sketch snapping produced floating-point noise (`-0.15000000000000002` instead of `-0.15`); snapping now divides by the grid count. Two outdated test expectations were corrected (`storage`: the nearest part to the selected base plate is `sensor-case`; `meshing`: the error text now says "isolated point contact").
- **Test status (measured in a Linux sandbox):** `sim-core` 71/71, `aerolab-data` 10/10, `aerolab-rules` 15/15, application 95/95. Not run on screen.

## 0.14.0 — Conforming local mesh refinement
- Added deterministic longest-edge bisection of marked triangular mesh elements.
- Updated every neighbour sharing a bisected edge so refinement remains conforming and does not create hanging nodes.
- Added axis-aligned local refinement regions, target edge lengths and a bounded iteration budget.
- Preserved per-element material, thickness and source-part provenance.
- Added mesh quality metrics (minimum angle, radius ratio, maximum edge and minimum area).
- Added a headless refinement example and regression tests for conformity, quality and conservation of area/volume.
- Added `Shift+M` in the Computation Lab to locally refine the centre of the workshop mesh before thermal analysis.


## 0.13.0 — Workshop plate meshing and unstructured thermal FEM
- Added a deterministic triangular mesher for adjacent coplanar rectangular workshop `Panel`/`Platform` parts; nodes are shared across matching boundaries.
- Added per-element material ID, thickness and source-part provenance.
- Added fail-fast checks for positive-area overlap, point-only/partial-edge contact, tilted/stacked geometry, invalid triangles, non-manifold edges and size limits.
- Added a Bevy-independent unstructured heat-conduction solver using linear triangular finite elements, lumped heat capacity and CSR stiffness assembly.
- Added fixed-temperature outer boundaries, volumetric source input, explicit stability validation, cancellation, progress and global energy diagnostics.
- Added an HTML/SVG mesh heat map and full-resolution nodal/element CSV output.
- Added an assembly adapter that converts coplanar horizontal workshop panels/platforms into a conforming mesh and reports unsupported component types.
- Added `M` to the Computation Lab to mesh the current workshop geometry and queue a background study.
- Added a two-material JSON input deck, a headless runner and adapter/solver/report regression tests.
- Limits: 2D plate meshes only; no stacked 3D bodies, arbitrary CAD, holes, contact resistance, anisotropic materials or certified engineering claims.


## 0.11.0 — Standalone 2D civil truss solver
- Added a Bevy-independent linear-elastic solver for pin-jointed civil trusses.
- Assembles a sparse global stiffness matrix and solves it with Jacobi-preconditioned conjugate gradients.
- Parallelizes row-wise matrix-vector products with Rayon while keeping deterministic row and dot-product ordering.
- Reports nodal displacement/reaction, member force/stress/strain, solver residual and strain-energy/external-work consistency.
- Added a civil bridge input deck, headless runner and analytical/reference tests.
- Explicitly documents the model limits: no bending, buckling, plasticity, fatigue, dynamics or 3D continuum FEA.

## 0.10.1 — Exact floating-point JSON round-tripping
- Enabled serde_json's `float_roundtrip` feature in both the application and the independent simulation crate.
- Scientific `f64` values now survive JSON save/load without the small last-bit changes produced by the default best-effort parser.
- Kept the strict equality assertion in the complete thermal-result round-trip test and added a focused regression test using representative IEEE-754 values.
- No numerical solver equations, input schemas or result schemas changed. The only runtime trade-off is slower JSON float parsing; it does not affect the solver's time-stepping loop.

## 0.10.0 — Independent scientific engine and finite-volume solver
- Extracted numerical code into `crates/sim-core`, a Bevy-independent Rust library.
- Replaced the unitless five-point stencil with transient 2D finite-volume conduction for a solid slab with SI dimensions and thickness.
- Added material catalogues and per-cell material maps with harmonic-mean interface conductivity.
- Added insulated and fixed-temperature boundaries, a volumetric heat source and a per-cell explicit stability check.
- Reports now include the final temperature field, simulated time, stability limit, initial/final energy, boundary/source energy and energy-balance residual.
- Added independent tests for conservation, uniform-field invariance, boundary cooling, heterogeneous materials, thread-count determinism, stability rejection and cancellation.
- Solver revision 2.1 precomputes per-cell heat capacities and internal/boundary face conductances once, reducing repeated material lookups in the parallel time-step loop.
- Uses compensated energy sums, a numerically safer harmonic mean and an incremental mean-temperature calculation.
- Updated the worker queue to persist full finite-volume result reports.
- The solver remains a generic material study and is not coupled to the 3D assembly or intended as a certified engineering model.

## 0.9.0 — Contract-to-delivery campaign loop

- Connected the three campus facilities into a playable project loop: accept contract, submit assembly/profile, run linked laboratory analysis, perform a field test, deliver the production batch and receive payment.
- Added contract-specific design checks for estimated assembly mass, civilian instrument power draw, operating-temperature envelope, uptime target, data completeness, storage and per-unit production budget.
- Added a configurable civilian instrument profile with keyboard controls and revision tracking.
- Linked background thermal-screening jobs to contract IDs; successful completion unlocks the field-test stage.
- Added a deterministic 1,000-sample instrument availability/data-completeness field-test report written to `results/`.
- Added production-cost deduction, client payout, net-profit history and a completed-contract ledger.
- Added atomic campaign save/load at `saves/campaign.json`, including offers, active/completed projects, credits, profile and PRNG state. Interrupted background analyses are recoverable as retryable jobs.
- Reduced default demonstrator and newly-created structural component dimensions to lightweight instrument-scale assemblies; lowered editable dimension minimum to 2 mm.
- Added tests for campaign serialization, design snapshots, deterministic contract generation and revised requirement ranges.
- Explicitly documented that the thermal-screening task is still a generic heat-diffusion demo and that the field-test model is a gameplay abstraction, not an engineering qualification standard.

## 0.8.0

- Replaced fixed demo contracts with a randomized board containing five scientific/civil-aerospace domains.
- Added seeded contract generation, shuffled domain presentation, unique offer IDs and repeatable tests.
- Added per-offer client, title, difficulty, unit count, instrument mass/power limits, operating-temperature interval, uptime and data-completeness thresholds, storage capacity, production budget, prospective reward and deadline.
- Added `N` to refresh the board while preserving active contracts.
- Expanded offer acceptance to keys `1` through `5`; accepted offers are removed from the available list.
- Added tests for domain coverage, deterministic generation, constraint ranges, refresh behavior and the active-contract limit.
- Contract families are deliberately non-weaponized: scientific sounding, long-duration observation, ground weather networks, cooperative civil-air telemetry and environmental air-ground mapping. No missile-class performance or weapon-system requirements are generated.
- Contract validation, payouts, deadline countdown and career persistence remain future work.


## 0.7.0 — Interactive island campus

- Added three clickable facilities: Contract Office, Assembly Workshop and Computation Lab.
- Added facility navigation from the island and Escape-to-return behavior.
- Added a prototype contract board with three generic research/equipment offers, duplicate protection and a two-active-contract limit.
- Connected the workshop to the existing assembly editor and visual sequencer.
- Connected the computation lab to the existing background thermal-diffusion research queue.
- Hide assembly/research/sequencer interfaces outside their corresponding facilities.
- Assembly component visuals are now spawned only while the workshop is open.
- Added campus behavior tests and documentation.

## 0.6.3 — Graph editor borrow-checker fixes
- Copied the hit-tested block ID and position before mutating `SequenceWorkspace`.
- Copied the drag offset before taking a mutable borrow of the selected block.
- Preserved graph hit-testing, drag offsets, and movement clamps; no save-schema changes.

## 0.6.2 — Bevy 0.20 input and window marker fixes
- Imported `bevy::window::PrimaryWindow` explicitly where primary-window queries are used.
- Changed the breakpoint shortcut to `KeyCode::KeyB`, the Bevy 0.20 physical key variant.
- Removed currently unused re-exports that caused compiler warnings.
- No runtime model or save-schema changes.

## 0.6.1 — Compile visibility and Bevy prelude fixes
- Replaced the narrow Bevy import list in the research job manager with `bevy::prelude::*`, covering `Color`, `default`, and related UI types used there.
- Made marker/resource types used in public Bevy system signatures visible (`ResearchHud`, editor markers/assets/revision, sequence HUD and graph markers).
- Kept the runtime behavior and serialized project schemas unchanged.
- This is a compile-fix release; a full Cargo build still needs to be run on a machine with the pinned Rust/Bevy toolchain.

## 0.6.0 — Visual sequence graph and debugging
- Added a draggable Bevy UI node canvas synchronized with the existing typed sequencer.
- Added visible sequential-flow links and TRUE branch links, with distinct colors.
- Persisted node positions in program schema 2 and added a version 1 to 2 migration.
- Added numeric variables, assignment, arithmetic expressions, increment and comparison-branch blocks for generic lab automation.
- Added per-block breakpoints and pause-before-execute behavior.
- Added a bounded execution trace and JSON export (`Ctrl+F7`).
- Added runtime variable values to the sequencer and graph inspectors.
- Avoided key collisions by activating sequencer editing shortcuts only while the graph editor is open.
- Included completed waits in the execution trace and added a variable-overflow guard.
- Added validation and runtime tests for variables, breakpoints and node coordinates.
- Kept the graph as an execution-flow editor; arbitrary port wiring and custom expression parsing remain future work.


## 0.5.0 — Laboratory block sequencer
- Added a typed sequence model with Start, Wait, temperature read, temperature condition, heater output, Record and End blocks.
- Added stable block identifiers and branch targets that survive block reordering.
- Added structural validation, protected Start/End blocks, protected branch targets and a 128-block editor limit.
- Added deterministic single-step and real-time run/pause execution.
- Added a generic lumped thermal-chamber demonstrator with closed-form time integration.
- Added bounded instruction execution to protect against runaway branch loops.
- Added JSON save/load for laboratory programs.
- Added a sequence HUD and keyboard editor controls, including condition comparison and jump-target editing.
- Added a serialized demo program and JSON round-trip test.
- Added tests for sequence validation, branch references, editing constraints, step execution, pause/resume and completion.
- Kept the sequencer scoped to generic laboratory automation; no missile-specific propulsion, guidance or flight-control logic is implemented.

## 0.4.0
- Added schema 2 assembly connections with schema 1 migration.
- Added directed parent-child graph validation: existing endpoints, unique connection IDs, one parent per child and cycle rejection.
- Added keyboard creation/removal of metadata-only assembly links.
- Added 64-snapshot in-memory undo/redo.
- Added 1 cm fine movement and dimension adjustment, plus 1-degree rotation mode.
- Added tests for schema migration, connection graph validation and undo/redo.
- Documented graph semantics and current limitations.

## 0.3.0
- Added SI quantity wrappers and simulation provenance metadata.
- Added configurable Rayon worker count and cooperative job cancellation.
- Added local-space picking for rotated components and independent axis resizing.

## 0.2.0
- Added interactive generic component editing and JSON save/load.

## 0.1.0
- Initial Bevy scene, assembly model and background thermal demonstration.


## 0.19.0

- Added local-axis translation arrows with conical heads and rotation rings to the Workshop viewport.
- Added mouse picking, hover highlighting and drag transforms with ray/plane projection.
- Added distance-aware gizmo sizing so manipulators remain readable across camera zoom levels.
- Added clickable Translate/Rotate mode controls with active-mode highlighting and `T` / `R` shortcuts.
- Captured the pointer during a gizmo drag and paused camera navigation until release.
- Added absolute-position grid snapping for gizmo translation.
- Made each mouse drag a single undoable transaction; no-op clicks do not create history entries.
- Kept the scientific simulation crate unchanged.


## 0.20.0

- Added a right-side numeric inspector in the Workshop view.
- Added keyboard entry for absolute position, rotation and dimension values.
- Added per-field decrement/increment controls with sensible SI-sized steps.
- Position/dimension inputs use metres; rotation inputs use degrees.
- Added finite-value checks, bounds, angle normalization and one-step undo support for each committed edit.
- Numeric entry bypasses grid snapping so typed values remain exact.
- Camera and editor shortcuts are suspended while a numeric field is active.
- Added tests for absolute numeric edits, undo, angle normalization and dimension bounds.


## 0.21.0 - Five-facility campus separation

- Expanded the campus from three interactive facilities to five: contracts, construction, research, programming and computation.
- Added dedicated navigation destinations and F1-F5 shortcuts for all facilities.
- Moved the visual sequence editor and runtime ownership to the Programming Studio.
- Kept scientific background jobs and the research-job HUD in the Computation Centre.
- Added the Research Centre department overview as the 0.22 research-system entry point.
- Updated architecture and campus documentation to state what is and is not separated in this release.
