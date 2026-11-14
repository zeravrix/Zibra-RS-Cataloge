package org.zeravrix.parsers.site.madara.en

import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.madara.MadaraParser

@MangaSourceParser("DRAGONTEA", "DragonTea", "en")
internal class DragonTea(context: MangaLoaderContext) :
	MadaraParser(context, MangaParserSource.DRAGONTEA, "dragontea.ink") {
	override val datePattern = "MM/dd/yyyy"
}
