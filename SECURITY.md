# Seguridad

P-Rune corre como administrador y modifica el sistema, así que cualquier problema de seguridad se toma
en serio.

## Cómo reportar una vulnerabilidad

**No abras un issue público.** Reportala en privado desde la pestaña **Security → Report a
vulnerability** de este repositorio (reporte privado de vulnerabilidades de GitHub).

Incluí, si podés:

- qué parte de la app está afectada (tweaks, historial, winget, disco, instalador…);
- los pasos para reproducirlo y qué puede lograr alguien que lo aproveche;
- la versión de P-Rune y de Windows.

Vas a recibir una respuesta lo antes posible. Cuando haya un arreglo publicado, se puede dar crédito en
las notas de la versión si lo querés.

## Versiones con soporte

Solo la última versión publicada recibe arreglos de seguridad.

## Qué está dentro del alcance

- Escalada de privilegios a través de P-Rune (por ejemplo, que un programa sin admin logre que P-Rune
  escriba en el registro o ejecute algo como administrador).
- Ejecución de código en la interfaz (XSS) o abuso de los comandos internos.
- Problemas en la verificación de winget, en el historial o en los instaladores.

Los falsos positivos de antivirus no son vulnerabilidades, pero si encontrás uno, avisá igual en un issue.
