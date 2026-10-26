<?php
namespace App;

use Lib\Paragraph;

function f(Paragraph $p): void
{
    $p->getTranslation('it')->/*@cursor:para_only*/onlyParagraph();
    $p->get('x')->/*@cursor:first*/first();
    $p->untyped()->/*@cursor:none*/anything();
}
