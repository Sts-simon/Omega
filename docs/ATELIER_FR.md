# Atelier de construction — version française

L'atelier est organisé autour de trois zones : la palette de pièces, la vue 3D et les panneaux de contrôle.

## Construire

1. Choisir une catégorie de pièces dans la palette.
2. Faire glisser une pièce vers un point d'attache lumineux de la vue 3D.
3. Sélectionner une pièce pour ouvrir son inspecteur et afficher ses points d'attache libres.
4. Utiliser **W** pour le déplacement et **R** pour la rotation, comme dans KSP.
5. Faire glisser une poignée colorée pour transformer la pièce : rouge = X, vert = Y, bleu = Z. En mode rotation, les anneaux représentent les trois axes. La transformation est validée au relâchement et constitue une seule action annulable.
6. Maintenir **Maj** pour désactiver l'accrochage de la transformation (déplacement libre ou rotation fine).
7. Pour une surface aérodynamique, **Q** et **E** conservent la rotation rapide par pas de 15°.
8. Régler ses paramètres avec les boutons moins et plus, ou avec la molette lorsque le pointeur est au-dessus de la pièce.
9. Déplacer une pièce existante pour déplacer également ses pièces enfants.

## Diamètres et coiffe

Les pièces à section fixe (capteurs, ordinateurs, séparateurs, modules de mission, charges et avionique) affichent un **Diamètre de montage (m)** réglable par pas de 5 cm dans l'inspecteur. Il change uniquement leur largeur pour les aligner sur le corps porteur ; la longueur et la masse restent celles de la pièce.

La **COIFFE** est une pièce de la palette Structure et charge. Elle se place au sommet comme les autres pièces et règle sa longueur, son diamètre de base et son profil (conique, ogival ou parabolique). Aucune coiffe n'est ajoutée automatiquement. Une fois posée, elle ferme le sommet : retirer la coiffe pour ajouter une autre pièce au-dessus, puis la replacer.

## Inspecteur numérique

La partie basse de la colonne de droite affiche la position locale (X, Y, Z en mètres) et la rotation locale (X, Y, Z en degrés) de la pièce sélectionnée.

- Cliquer une valeur, taper un nombre décimal puis **Entrée** pour la valider.
- Appuyer sur **Échap** pour annuler. Une saisie vide, non numérique, infinie ou hors limites est refusée et la dernière valeur valide reste affichée.
- Les boutons **−** et **+** utilisent la précision choisie : **GROSSIER** (0,50 m / 15°), **NORMAL** (0,05 m / 5°), **FIN** (0,01 m / 1°) ou **TRÈS FIN** (0,001 m / 0,1°).
- Le bouton **0** remet un axe à zéro ; **TRANSLATIONS 0** et **ROTATIONS 0** remettent les trois axes correspondants ; **RESTAURER LA TRANSFORMATION INITIALE** ramène le montage par défaut.
- Chaque validation, pas ou remise à zéro est une seule opération annulable. Un glissement de gizmo met les six valeurs à jour immédiatement et n'enregistre qu'une opération au relâchement.
- La pièce racine reste verrouillée : aucun champ n'est proposé pour elle.

Le panneau est défilant pour les fenêtres courtes et les conceptions comportant beaucoup de pièces.

## Comprendre l'assemblage

L'arbre à droite affiche le nom lisible de chaque pièce, son point d'attache et son étage. Sélectionner une ligne recentre l'inspection sur cette pièce. Les séparateurs définissent les transitions entre étages.

## Contrôles

- **W** : outil de déplacement sur les axes X, Y et Z.
- **R** : outil de rotation autour des axes X, Y et Z.
- **F** : centrer la caméra sur la pièce sélectionnée.
- **Début (Home)** : cadrer l'ensemble du véhicule.
- **X** : changer le nombre de copies symétriques.
- **Suppr** : supprimer la pièce sélectionnée et son sous-ensemble.
- **Ctrl + Z** : annuler.
- **Ctrl + Y** : rétablir.
- **Clic droit + glisser** : tourner la caméra.
- **Clic molette + glisser** : déplacer la caméra.
- **Molette** : zoomer, sauf lorsque le pointeur est au-dessus d'une pièce sélectionnée.

## Validation

Les erreurs empêchent les opérations incompatibles avec les règles d'assemblage. Les avertissements signalent les compromis de conception sans bloquer la construction. Les lignes associées à une pièce peuvent être sélectionnées pour retrouver rapidement l'élément concerné.

Le filtre de catégories ne supprime aucune pièce et ne modifie jamais les données du véhicule.
