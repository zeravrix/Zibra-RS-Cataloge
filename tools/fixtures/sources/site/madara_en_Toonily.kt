package org.zeravrix.parsers.site.madara.en

import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.madara.MadaraParser

@MangaSourceParser("TOONILY", "Toonily", "en")
internal class Toonily(context: MangaLoaderContext) :
	MadaraParser(context, MangaParserSource.TOONILY, "toonily.com", pageSize = 18) {
	override val listUrl = "webtoon/"
	override val tagPrefix = "webtoon-genre/"
	override val datePattern = "MMMM dd, yyyy"
}
